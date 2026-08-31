// The hardening every host of the shared editor bundle applies, the message composer AND the
// Settings signature editor. It lives here, in ONE place, because it is a security contract rather
// than a per-screen detail: authoring a signature is authoring mail content, so it gets the
// composer's gates, not a lighter set (docs/composer-security.md, "Layer 3, Native WebView host
// gates"; docs/signatures.md). Two hosts with two copies of these settings is two chances for one of
// them to drift, which is exactly why Android collapsed its two into EditorWebView.kt.
//
// What it guarantees: script runs for the bundled local document only, no host objects, no web
// messages, no default context menu, no new windows, every navigation away is cancelled, and every
// http/https subresource request is answered 403, the native barrier behind the bundle's own CSP.
//
// Do not relax one of these without updating that doc (rule AND matrix) and every other platform.

using System.Text.Json;
using Microsoft.UI.Xaml.Controls;
using Microsoft.Web.WebView2.Core;

namespace Allodia.Mailcal.Services;

/// <summary>
/// One hardened host of <c>clients/composer/dist/editor.html</c>. Owns the WebView2's gate wiring and
/// the single navigation the host is allowed to perform, so a caller only ever loads the bundle and
/// talks to it through <c>window.*</c> hooks.
/// </summary>
internal sealed class EditorWebViewHost
{
    private readonly WebView2 _view;
    private Task? _init;

    // The one navigation this host permits: our own NavigateToString of the bundle. Everything
    // after it is cancelled, so a link, a redirect, or a script-driven location change cannot take
    // the editor anywhere.
    private bool _expectingLoad;

    /// <summary>Wraps <paramref name="view"/>; the gates are applied on the first load.</summary>
    internal EditorWebViewHost(WebView2 view) => _view = view;

    /// <summary>
    /// Invoked once the bundle has finished loading, at which point its <c>window.*</c> hooks are
    /// defined. Every seed a host injects belongs here and nowhere earlier: a hook called before the
    /// page has parsed lands on an undefined function and fails silently, leaving the editor in its
    /// default state with no error anywhere.
    /// </summary>
    internal Func<Task>? PageReady { get; set; }

    /// <summary>The underlying WebView2 environment, or <c>null</c> before the first load. Callers
    /// use it to tell "the page is up" from "nothing has loaded yet".</summary>
    internal CoreWebView2? Core => _view.CoreWebView2;

    /// <summary>Applies the gates (once) and loads the shared editor bundle.</summary>
    internal async Task LoadAsync()
    {
        await EnsureAsync();
        _expectingLoad = true;
        _view.CoreWebView2!.NavigateToString(Bundle());
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

    /// <summary>Releases the WebView2 backing this host. Safe to call on one that never
    /// initialised.</summary>
    internal void Close()
    {
        try
        {
            _view.Close();
        }
        catch (Exception ex)
        {
            // A host torn down before its WebView2 ever initialised has nothing to close.
            Log.Warn($"editor: teardown ({ex.GetType().Name})");
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
        settings.IsWebMessageEnabled = false;
        settings.AreDefaultContextMenusEnabled = false;
        core.NavigationStarting += OnNavigationStarting;
        core.NavigationCompleted += OnNavigationCompleted;
        core.NewWindowRequested += (_, args) => args.Handled = true;
        core.AddWebResourceRequestedFilter("*", CoreWebView2WebResourceContext.All);
        core.WebResourceRequested += OnWebResourceRequested;
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
        if (!args.IsSuccess || PageReady is not { } ready)
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
