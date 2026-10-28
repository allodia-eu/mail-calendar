// The reading pane's hardened WebView2 host: the native half of the message-body security gates.
// Split out of ReadingView.xaml.cs to keep that file under the 500-line limit, and because these
// gates are worth reading as one thing.
//
// THE GATES HERE ARE A CROSS-PLATFORM CONTRACT, see docs/rendering-security.md. Any gate added or
// raised on one platform must be applied to ALL of them, and recorded there, in the same change.
//
// The full HTML document (strict CSP, base styling, remote-image gating) is built in shared Rust
// (MailboxModel.RenderMessageHtml), so every client behaves identically. What is unavoidably
// native lives here: scripting off, no host bridge, in-view navigation blocked (a clicked link
// goes to the default browser instead), no popups, and a WebResourceRequested barrier that hard-
// blocks remote http(s) sub-resources until the user opts into images, defence in depth atop the
// document CSP, the WebView2 twin of Android's shouldInterceptRequest gate.

using System.Globalization;
using System.Text.Json;
using Allodia.Mailcal.Services;
using Microsoft.UI.Xaml;
using Microsoft.Web.WebView2.Core;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Views;

public sealed partial class ReadingView
{
    /// <summary>The fragment + load-images choice last rendered, so an unrelated re-render
    /// doesn't rebuild the document (an FFI call) or reload the page. Null forces a render.</summary>
    private string? _lastFragment;
    private bool _lastLoadRemoteImages;
    private MessageScheme _lastScheme;

    /// <summary>Where the reader was when the same message was re-rendered in another scheme, put
    /// back once the new document's DOM is in, before its first paint (docs/reading-dark-mode.md,
    /// rule 9). Null when there is nothing to restore.</summary>
    private (double X, double Y)? _keptPlace;

    /// <summary>One-shot CoreWebView2 init (settings + navigation lock), awaited per HTML render.</summary>
    private Task? _coreInit;

    /// <summary>Set just before our own NavigateToString so the navigation lock lets it through.</summary>
    private bool _expectingLoad;

    /// <summary>
    /// Unloads the message body while the composer has the detail column (MainWindow.Compose.cs).
    /// The composer hosts a WebView2 of its own, and leaving this one loaded behind it would keep a
    /// second document, and, if the user had opted into this message's remote images, its remote
    /// content, alive under a pane nobody can see it through. Clearing the render guard makes the
    /// next <c>Render</c> rebuild it from the model, so <see cref="ResumeBody"/> costs one
    /// re-render and nothing is refetched.
    /// </summary>
    internal void SuspendBody()
    {
        _lastFragment = null;
        // The body is about to be blanked, so there is nothing left for the next message to be
        // handed over from, ResumeBody re-renders from the model instead.
        _handover.Cleared();
        if (Body.CoreWebView2 is null)
        {
            return; // never rendered a body, nothing to unload.
        }
        try
        {
            _expectingLoad = true;
            Body.CoreWebView2.NavigateToString("<!doctype html><html><body></body></html>");
        }
        catch (Exception ex)
        {
            _expectingLoad = false;
            Log.Warn($"reading: couldn't unload the body for the composer ({ex.GetType().Name})");
        }
    }

    /// <summary>Restores the message body after the composer closes.</summary>
    internal void ResumeBody() => Render();

    // Wrap the sanitised fragment in the shared-Rust document and load it into the hardened
    // WebView2. Skip when the inputs are unchanged, so unrelated re-renders don't rebuild the
    // document (an FFI call) or reload the page, only the fragment, the load-images choice or the
    // scheme matters. A change of scheme alone keeps the reader's place.
    private async void RenderHtml(string fragment)
    {
        var scheme = Scheme;
        var loadRemoteImages = _loadRemoteImages;
        var sameMessage = _lastFragment == fragment && _lastLoadRemoteImages == loadRemoteImages;
        if (sameMessage && _lastScheme == scheme)
        {
            return;
        }
        _lastFragment = fragment;
        _lastLoadRemoteImages = loadRemoteImages;
        _lastScheme = scheme;
        var document = _model!.RenderMessageHtml(fragment, loadRemoteImages, scheme);
        try
        {
            await EnsureCoreAsync();
            var place = sameMessage ? await ReadPlaceAsync() : null;
            // A newer render started while the place was being read; it loads its own document,
            // and this place must not land on it.
            if (_lastFragment != fragment || _lastLoadRemoteImages != loadRemoteImages
                || _lastScheme != scheme)
            {
                return;
            }
            _keptPlace = place;
            _expectingLoad = true;
            Log.Debug("reading: loading the body into the web view");
            Body.CoreWebView2!.NavigateToString(document);
        }
        catch (Exception ex)
        {
            // No WebView2 runtime, or the document exceeded NavigateToString's limit: fall
            // back to plain text rather than showing a blank pane. Clear the guard so a later
            // attempt re-renders.
            _expectingLoad = false;
            _lastFragment = null;
            Log.Warn($"reading: couldn't render HTML in WebView2 ({ex.Message}); falling back to text");
            FallBackToText();
        }
    }

    private void FallBackToText()
    {
        var plain = BodySnapshot?.Plain;
        RemoteImagesBanner.Visibility = Visibility.Collapsed;
        if (!string.IsNullOrEmpty(plain))
        {
            LinkedTextBlock.Fill(PlainText, plain);
            ShowState(plain: true);
        }
        else
        {
            EmptyText.Text = L10n.ReadingWebviewUnavailable();
            ShowState(empty: true);
        }
    }

    private Task EnsureCoreAsync() => _coreInit ??= InitCoreAsync();

    /// <summary>
    /// The page's scroll position, read by script the app runs: WebView2 offers no native one, and
    /// <c>IsScriptEnabled = false</c> stops only the document's own script, never
    /// <c>ExecuteScriptAsync</c> (docs/rendering-security.md, gate 1). Null at the very top, where
    /// there is nothing to restore, and when the read fails.
    /// </summary>
    private async Task<(double X, double Y)?> ReadPlaceAsync()
    {
        try
        {
            var json = await Body.CoreWebView2!.ExecuteScriptAsync("[window.scrollX, window.scrollY]");
            var point = JsonSerializer.Deserialize<double[]>(json);
            return point is [var x, var y] && (x != 0 || y != 0) ? (x, y) : null;
        }
        catch (Exception ex)
        {
            Log.Debug($"reading: couldn't read the scroll position ({ex.GetType().Name})");
            return null;
        }
    }

    // The new document's DOM is in and nothing has been painted yet: put the reader back.
    private async void OnDomContentLoaded(CoreWebView2 sender, CoreWebView2DOMContentLoadedEventArgs args)
    {
        if (_keptPlace is not { } place)
        {
            return;
        }
        _keptPlace = null;
        var x = place.X.ToString(CultureInfo.InvariantCulture);
        var y = place.Y.ToString(CultureInfo.InvariantCulture);
        try
        {
            await sender.ExecuteScriptAsync($"window.scrollTo({x}, {y})");
        }
        catch (Exception ex)
        {
            Log.Debug($"reading: couldn't restore the scroll position ({ex.GetType().Name})");
        }
    }

    private async Task InitCoreAsync()
    {
        Log.Debug("reading: creating the web view");
        await Body.EnsureCoreWebView2Async();
        Log.Debug("reading: web view created");
        var core = Body.CoreWebView2;
        var settings = core.Settings;
        // Defence in depth atop the core's sanitisation: no scripting, no host bridge.
        settings.IsScriptEnabled = false;
        settings.AreHostObjectsAllowed = false;
        settings.AreDevToolsEnabled = WebViewInspection.DevToolsEnabled;
        settings.IsWebMessageEnabled = false;
        settings.AreDefaultContextMenusEnabled = false;
        // The reader's zoom: touch pinch, and Ctrl+scroll / Ctrl +/- (docs/reading-zoom.md). Both
        // default to true, and are stated anyway, because this file is where the reading host's
        // behaviour is read off and a default a later WebView2 changes would take the pinch away
        // silently. What cannot be stated here is rule 5's reset per message: the zoom factor lives
        // on CoreWebView2Controller, which the WinUI 3 WebView2 element does not surface, so a zoom
        // outlives the message it was made on. Recorded under that contract's "Known gaps".
        settings.IsPinchZoomEnabled = true;
        settings.IsZoomControlEnabled = true;
        // Block in-view navigations; allow only our NavigateToString. A clicked link opens
        // in the default browser instead (OnNavigationStarting).
        core.NavigationStarting += OnNavigationStarting;
        // A scheme change keeps the reader's place (RenderHtml).
        core.DOMContentLoaded += OnDomContentLoaded;
        // Paired with the "loading" line above, this is how long a body takes to appear. It is also
        // what separates the two halves of this element in the window-order fault: NAVIGATING is
        // not part of it and completes long after, while CREATING the view is the interval the
        // mailbox takes the foreground in (docs/client-traps.md).
        core.NavigationCompleted += (_, _) => Log.Debug("reading: the body finished loading");
        // Never open popups / new windows in-app; a target=_blank link the user clicked is
        // surfaced here rather than as a navigation, so open it in the default browser too.
        core.NewWindowRequested += (_, args) =>
        {
            if (args.IsUserInitiated)
            {
                TryOpenExternally(args.Uri);
            }
            args.Handled = true;
        };
        // Second barrier to the document CSP: hard-block remote http(s) sub-resource loads
        // (images, fonts, CSS) unless the user opted into images. The NavigateToString document
        // itself isn't an http(s) request, so it passes through.
        core.AddWebResourceRequestedFilter("*", CoreWebView2WebResourceContext.All);
        core.WebResourceRequested += OnWebResourceRequested;
    }

    private void OnNavigationStarting(CoreWebView2 sender, CoreWebView2NavigationStartingEventArgs args)
    {
        // Our own NavigateToString load is expected once; everything else (link taps,
        // redirects, remote loads) is cancelled, the body is inert. Sub-resources (images)
        // aren't navigations; the document CSP + the resource filter below gate those.
        if (_expectingLoad)
        {
            _expectingLoad = false;
            return;
        }
        // A link the user clicked opens in their default browser/handler instead; the in-view
        // navigation is still cancelled, so the document stays inert.
        if (args.IsUserInitiated)
        {
            TryOpenExternally(args.Uri);
        }
        args.Cancel = true;
    }

    // Hand a clicked link's URL to the OS default handler. Whether to open it is the
    // shared-Rust launch policy (a strict scheme allowlist, http(s)/mailto/tel only; mail is
    // hostile input) so every client is identical and consistent with what the sanitiser
    // keeps; never data:/file:/custom schemes. See docs/rendering-security.md.
    private void TryOpenExternally(string? uri)
    {
        if (!string.IsNullOrEmpty(uri)
            && _model?.ShouldOpenExternalLink(uri) == true
            && Uri.TryCreate(uri, UriKind.Absolute, out var parsed))
        {
            _ = Windows.System.Launcher.LaunchUriAsync(parsed);
        }
    }

    private void OnWebResourceRequested(CoreWebView2 sender, CoreWebView2WebResourceRequestedEventArgs args)
    {
        if (_loadRemoteImages)
        {
            return;
        }
        var uri = args.Request.Uri ?? string.Empty;
        if (uri.StartsWith("http://", StringComparison.OrdinalIgnoreCase)
            || uri.StartsWith("https://", StringComparison.OrdinalIgnoreCase))
        {
            // Empty 403, the remote resource never loads until the user opts in.
            args.Response = sender.Environment.CreateWebResourceResponse(null, 403, "Blocked", string.Empty);
        }
    }
}
