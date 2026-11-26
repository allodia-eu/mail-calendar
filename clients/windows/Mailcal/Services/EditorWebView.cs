// The hardening every host of the shared editor bundle applies, the message composer AND the
// Settings signature editor. It lives here, in ONE place, because it is a security contract rather
// than a per-screen detail: authoring a signature is authoring mail content, so it gets the
// composer's gates, not a lighter set (docs/composer-security.md, "Layer 3, Native WebView host
// gates"; docs/signatures.md). Two hosts with two copies of these settings is two chances for one of
// them to drift, which is exactly why Android collapsed its two into EditorWebView.kt.
//
// What it guarantees: script runs for the bundled local document only, no host objects, web
// messages only for the editor's request channel and only where a host answers it (each message
// parsed by the core, Gate 2), a context menu filtered down to the editing actions, no new windows,
// every navigation away is cancelled, and every http/https subresource request is answered 403, the
// native barrier behind the bundle's own CSP.
//
// Do not relax one of these without updating that doc (rule AND matrix) and every other platform.

using System.ComponentModel;
using System.Text.Json;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.Web.WebView2.Core;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

/// <summary>
/// One hardened host of <c>clients/composer/dist/editor.html</c>. Owns the WebView2's gate wiring and
/// the single navigation the host is allowed to perform, so a caller only ever loads the bundle and
/// talks to it through <c>window.*</c> hooks.
/// </summary>
internal sealed class EditorWebViewHost
{
    // How long the right-click menu waits for the page to say what there is to undo and redo.
    private static readonly TimeSpan HistoryAnswerTimeout = TimeSpan.FromMilliseconds(300);

    private readonly WebView2 _view;
    private Task? _init;

    // The one navigation this host permits: our own NavigateToString of the bundle. Everything
    // after it is cancelled, so a link, a redirect, or a script-driven location change cannot take
    // the editor anywhere.
    private bool _expectingLoad;

    // Set once the bundle has loaded, so its hooks exist.
    private bool _loaded;

    private MailboxModel? _model;

    // The page the loaded document was last put on, so a Settings signal that moved something
    // else does not name it again.
    private MessageScheme? _namedPage;

    // The script registered to run before each document this view loads, removed before the next
    // load registers the state as it is then.
    private string? _preload;

    // Cancelled by Close. A registration still pending when the view closes may never complete,
    // even when asked to cancel, and while it is awaited it holds this host, the view and whatever
    // owns them, so a wait on one lets go of it when the view closes.
    private readonly CancellationTokenSource _closing = new();

    /// <summary>Wraps <paramref name="view"/>; the gates are applied on the first load.</summary>
    internal EditorWebViewHost(WebView2 view)
    {
        _view = view;
        _view.ActualThemeChanged += (_, _) => _ = SyncSchemeAsync();
    }

    /// <summary>
    /// The model whose dark-messages preference, with the app's scheme, decides the page the message
    /// is written on exactly as it decides a reading view's (docs/reading-dark-mode.md, "Writing in
    /// a dark app"), and which stores the page the toolbar's sun or moon asks for. A host that
    /// leaves it unset keeps the message on the editor's own light page. Set before the first load,
    /// which turns the request channel on for the page's colours; the host announces them once the
    /// page has loaded.
    /// </summary>
    internal MailboxModel? Model
    {
        get => _model;
        set
        {
            if (_model is { } old)
            {
                old.PropertyChanged -= OnModelChanged;
            }
            _model = value;
            if (value is not null)
            {
                value.PropertyChanged += OnModelChanged;
            }
        }
    }

    private bool AppDark => _view.ActualTheme == ElementTheme.Dark;

    /// <summary>The page the message is written on, or null for a host that names none.</summary>
    private MessageScheme? Page =>
        _model is { } model ? MessageSchemes.For(AppDark, model.DarkMessageBackground) : null;

    /// <summary>
    /// Invoked once the bundle has finished loading, at which point its <c>window.*</c> hooks are
    /// defined. Every seed a host injects belongs here and nowhere earlier: a hook called before the
    /// page has parsed lands on an undefined function and fails silently, leaving the editor in its
    /// default state with no error anywhere.
    /// </summary>
    internal Func<Task>? PageReady { get; set; }

    /// <summary>
    /// Receives the editor's requests, each already parsed by the core. Set before the first load
    /// to turn the channel on; a host that leaves it unset keeps web messages off altogether.
    /// </summary>
    internal Action<ComposerHostRequest>? HostRequested { get; set; }

    /// <summary>Every message the page posts, as it came, before <see cref="HostRequested"/> is
    /// told what the core parsed of it.</summary>
    internal Action<string>? Posted { get; set; }

    /// <summary>The underlying WebView2 environment, or <c>null</c> before the first load. Callers
    /// use it to tell "the page is up" from "nothing has loaded yet".</summary>
    internal CoreWebView2? Core => _view.CoreWebView2;

    /// <summary>Applies the gates (once) and loads the shared editor bundle.</summary>
    internal async Task LoadAsync()
    {
        await EnsureAsync();
        try
        {
            await InTreeAsync();
            await PreloadAsync();
        }
        catch (OperationCanceledException) when (_closing.IsCancellationRequested)
        {
            return;
        }
        _expectingLoad = true;
        _view.CoreWebView2!.NavigateToString(Bundle());
    }

    // A host loads its editor before it puts the view in a window, and until then the view's
    // ActualTheme is the default, light, rather than the app's.
    private Task InTreeAsync()
    {
        if (_view.IsLoaded)
        {
            return Task.CompletedTask;
        }
        var inTree = new TaskCompletionSource();
        void OnLoaded(object sender, RoutedEventArgs e)
        {
            _view.Loaded -= OnLoaded;
            inTree.TrySetResult();
        }
        _view.Loaded += OnLoaded;
        _closing.Token.Register(() => inTree.TrySetCanceled(_closing.Token));
        return inTree.Task;
    }

    // Hands the app's scheme and the page to the document before it runs, so the editor's first
    // frame is drawn on them rather than on the light page the stylesheet starts on. WebView2 keeps
    // a registration for every later document, so the one made for an earlier load goes first.
    private async Task PreloadAsync()
    {
        var core = _view.CoreWebView2!;
        if (_preload is { } previous)
        {
            core.RemoveScriptToExecuteOnDocumentCreated(previous);
            _preload = null;
        }
        _preload = await core.AddScriptToExecuteOnDocumentCreatedAsync(
            ComposerPage.Preload(AppDark, Page, MailcalBindingsMethods.ComposerPagePreloadScript)).AsTask().WaitAsync(_closing.Token);
    }

    /// <summary>Runs <paramref name="script"/> in the editor document, discarding its result.</summary>
    internal async Task RunAsync(string script)
    {
        await EnsureAsync();
        await _view.CoreWebView2!.ExecuteScriptAsync(script);
    }

    /// <summary>Runs <paramref name="script"/> and decodes its result as a JavaScript string,
    /// what every hook that hands data back (<c>composerDocument()</c>, <c>signatureBody()</c>)
    /// returns.</summary>
    internal async Task<string?> ReadStringAsync(string script)
    {
        await EnsureAsync();
        var encoded = await _view.CoreWebView2!.ExecuteScriptAsync(script);
        return JsonSerializer.Deserialize<string>(encoded);
    }

    /// <summary>Runs <paramref name="script"/> and decodes its result as a number, which is what
    /// <c>composerRevision()</c> answers. <c>-1</c> when the bundle has not parsed yet, so a host
    /// sampling it cannot mistake a page that is still loading for one nobody has typed in.</summary>
    internal async Task<int> ReadNumberAsync(string script)
    {
        await EnsureAsync();
        var encoded = await _view.CoreWebView2!.ExecuteScriptAsync(script);
        return int.TryParse(encoded, out var value) ? value : -1;
    }

    /// <summary>Releases the WebView2 backing this host. Safe to call on one that never
    /// initialised.</summary>
    internal void Close()
    {
        _loaded = false;
        _closing.Cancel();
        // The model is the app's and outlives this view.
        Model = null;
        try
        {
            _view.Close();
        }
        catch (Exception ex)
        {
            // A host torn down before its WebView2 ever initialised has nothing to close.
            Log.Warn($"editor: teardown ({ex.GetType().Name})");
        }
        ReleaseProbe.Watch(_view, "editor");
    }

    /// <summary>The script naming the app's light or dark, which the editor's chrome follows
    /// (<c>clients/composer/src/scheme.ts</c>).</summary>
    internal static string SchemeScript(bool dark) => $"window.setComposerScheme({Arg(dark ? "dark" : "light")})";

    // Names the scheme from the element's ActualTheme, which carries an appearance chosen in the app,
    // and the page the message is written on, which that scheme moves. WebView2's own
    // `prefers-color-scheme` follows the system's instead, so it is not consulted.
    private async Task SyncSchemeAsync()
    {
        if (!_loaded || _view.CoreWebView2 is not { } core)
        {
            return;
        }
        var page = Page;
        var script = SchemeScript(AppDark);
        if (page is { } scheme)
        {
            script += ";" + MailcalBindingsMethods.ComposerPageScript(scheme);
        }
        _namedPage = page;
        try
        {
            await core.ExecuteScriptAsync(script);
        }
        catch (Exception ex)
        {
            Log.Warn($"editor: colour scheme ({ex.GetType().Name})");
        }
    }

    // The dark-messages preference arrives with the core's Settings signal, from the reading view's
    // toggle, Settings → Reading or this editor's own toolbar.
    private void OnModelChanged(object? sender, PropertyChangedEventArgs e)
    {
        if (e.PropertyName == nameof(MailboxModel.DisplaySettingsVersion) && Page != _namedPage)
        {
            _ = SyncSchemeAsync();
        }
    }

    /// <summary>Encodes <paramref name="value"/> as a JavaScript string literal, so host-supplied
    /// text can be passed into a hook without breaking out of the argument.</summary>
    internal static string Arg(string value) => JsonSerializer.Serialize(value);

    private Task EnsureAsync() => _init ??= InitAsync();

    private async Task InitAsync()
    {
        await _view.EnsureCoreWebView2Async();
        var core = _view.CoreWebView2;
        var settings = core.Settings;
        // Script runs ONLY for the local editor document; neither host ever renders untrusted mail
        // in this WebView2 (the reading pane keeps its own, with scripting off).
        settings.IsScriptEnabled = true;
        settings.AreHostObjectsAllowed = false;
        settings.AreDevToolsEnabled = WebViewInspection.DevToolsEnabled;
        // A host that names a page answers the colours on it, so it listens even with no requests
        // of its own (the signature editor).
        var listens = HostRequested is not null || _model is not null;
        settings.IsWebMessageEnabled = listens;
        if (listens)
        {
            core.WebMessageReceived += OnWebMessageReceived;
        }
        // On, but filtered: `OnContextMenuRequested` keeps only the editing actions. Left off, a
        // right-click did nothing at all, and a link in a quoted original was text the user could
        // see and not copy.
        settings.AreDefaultContextMenusEnabled = true;
        core.ContextMenuRequested += OnContextMenuRequested;
        core.NavigationStarting += OnNavigationStarting;
        core.NavigationCompleted += OnNavigationCompleted;
        core.NewWindowRequested += (_, args) => args.Handled = true;
        core.AddWebResourceRequestedFilter("*", CoreWebView2WebResourceContext.All);
        core.WebResourceRequested += OnWebResourceRequested;
    }

    // WebView2's default menu carries items this composer must not offer: opening a link,
    // saving one, reloading the document, and Inspect Element. Filtering rather than replacing
    // keeps each remaining item the platform's, with its own label and accelerator. Undo and Redo
    // are the exception (EditorMenu.Classify says why): each is replaced in its own place.
    private static void OnContextMenuRequested(
        CoreWebView2 sender,
        CoreWebView2ContextMenuRequestedEventArgs args)
    {
        var items = args.MenuItems;
        var history = new List<(int At, HistoryStep Step, string Label)>();
        var kept = 0;
        for (var index = 0; index < items.Count;)
        {
            var name = items[index].Name;
            if (EditorMenu.Keeps(name))
            {
                kept++;
                index++;
                continue;
            }
            if (EditorMenu.Step(name) is { } step)
            {
                history.Add((kept + history.Count, step, items[index].Label));
            }
            items.RemoveAt(index);
        }
        if (history.Count > 0)
        {
            _ = AddHistoryItemsAsync(sender, items, history, args.GetDeferral());
        }
    }

    // The menu waits on the deferral, so the page is asked what there is to undo before it opens.
    // A page too busy to answer in time gets both items disabled rather than a menu that never opens.
    private static async Task AddHistoryItemsAsync(
        CoreWebView2 sender,
        IList<CoreWebView2ContextMenuItem> items,
        List<(int At, HistoryStep Step, string Label)> history,
        Windows.Foundation.Deferral deferral)
    {
        try
        {
            var asked = sender.ExecuteScriptAsync("window.composerHistorySteps?.()").AsTask();
            var answered = await Task.WhenAny(asked, Task.Delay(HistoryAnswerTimeout)) == asked;
            var steps = EditorMenu.ParseSteps(answered ? await asked : null);
            foreach (var (at, step, label) in history)
            {
                var item = sender.Environment.CreateContextMenuItem(label, null, CoreWebView2ContextMenuItemKind.Command);
                item.IsEnabled = steps.Contains(step);
                item.CustomItemSelected += (_, _) => _ = StepHistoryAsync(sender, step);
                items.Insert(Math.Min(at, items.Count), item);
            }
        }
        catch (Exception ex)
        {
            // A menu without Undo and Redo is still a menu; one held open forever is not.
            Log.Warn($"editor: history menu items ({ex.GetType().Name})");
        }
        finally
        {
            deferral.Complete();
        }
    }

    private static async Task StepHistoryAsync(CoreWebView2 sender, HistoryStep step)
    {
        try
        {
            await sender.ExecuteScriptAsync(EditorMenu.Script(step));
        }
        catch (Exception ex)
        {
            Log.Warn($"editor: {step} from the menu ({ex.GetType().Name})");
        }
    }

    /// A message that is not a string, or not a request the core has a type for, is dropped.
    private void OnWebMessageReceived(CoreWebView2 sender, CoreWebView2WebMessageReceivedEventArgs args)
    {
        string message;
        try
        {
            message = args.TryGetWebMessageAsString();
        }
        catch (ArgumentException)
        {
            return;
        }
        // Every message, the ones the parser leaves out included: the reply-draft report is the
        // core's to read (MailcalApp.composer_reply_draft_report).
        Posted?.Invoke(message);
        // The page's two are the core's: the colours it shows, and the sun or moon, whose Settings
        // signal names the page back to every editor and reading view.
        Func<string, bool>? choose = _model is { } model ? model.ComposerBackgroundChoice : null;
        switch (ComposerPage.Route(message, MailcalBindingsMethods.ComposerColoursAnswerScript, choose, out var answer))
        {
            case EditorPost.Answered:
                _ = AnswerAsync(sender, answer!);
                return;
            case EditorPost.Chosen:
                return;
        }
        if (MailcalBindingsMethods.ParseComposerHostRequest(message) is { } request)
        {
            HostRequested?.Invoke(request);
        }
    }

    private static async Task AnswerAsync(CoreWebView2 sender, string answer)
    {
        try
        {
            await sender.ExecuteScriptAsync(answer);
        }
        catch (Exception ex)
        {
            Log.Warn($"editor: page colours ({ex.GetType().Name})");
        }
    }

    private void OnNavigationStarting(CoreWebView2 sender, CoreWebView2NavigationStartingEventArgs args)
    {
        if (_expectingLoad)
        {
            _expectingLoad = false;
            return;
        }
        args.Cancel = true;
    }

    private async void OnNavigationCompleted(CoreWebView2 sender, CoreWebView2NavigationCompletedEventArgs args)
    {
        if (!args.IsSuccess)
        {
            return;
        }
        _loaded = true;
        // Before the seeds, so the chrome is in the app's scheme by the time anything is drawn in it.
        await SyncSchemeAsync();
        if (PageReady is not { } ready)
        {
            return;
        }
        try
        {
            await ready();
        }
        catch (Exception ex)
        {
            // The host's own seeding failed. Never let it escape an async void handler, which would
            // take the process down; the host reports what it could not do.
            Log.Warn($"editor: page-ready seeding ({ex.GetType().Name})");
        }
    }

    private void OnWebResourceRequested(CoreWebView2 sender, CoreWebView2WebResourceRequestedEventArgs args)
    {
        var uri = args.Request.Uri ?? string.Empty;
        if (uri.StartsWith("http://", StringComparison.OrdinalIgnoreCase)
            || uri.StartsWith("https://", StringComparison.OrdinalIgnoreCase))
        {
            args.Response = sender.Environment.CreateWebResourceResponse(null, 403, "Blocked", string.Empty);
        }
    }

    // The shared bundle, copied next to the exe by the csproj. The fallback defines both hosts'
    // read-back hooks so a missing asset degrades to an empty document rather than an exception at
    // the first ExecuteScriptAsync.
    private static string Bundle()
    {
        var asset = Path.Combine(AppContext.BaseDirectory, "composer", "editor.html");
        if (File.Exists(asset))
        {
            return File.ReadAllText(asset);
        }
        return "<!doctype html><html><body><script>"
            + "window.composerDocument=function(){return JSON.stringify({blocks:[],attachments:[]});};"
            + "window.signatureBody=function(){return JSON.stringify({body_html:\"\",body_plain:\"\"});};"
            + "</script></body></html>";
    }
}
