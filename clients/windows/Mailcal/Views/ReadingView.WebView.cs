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

using Allodia.Mailcal.Services;
using Microsoft.UI.Xaml;
using Microsoft.Web.WebView2.Core;

namespace Allodia.Mailcal.Views;

public sealed partial class ReadingView
{
    /// <summary>The fragment + load-images choice last rendered, so an unrelated re-render
    /// doesn't rebuild the document (an FFI call) or reload the page. Null forces a render.</summary>
    private string? _lastFragment;
    private bool _lastLoadRemoteImages;

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
    // document (an FFI call) or reload the page, only the fragment or the load-images choice
    // matters.
    private async void RenderHtml(string fragment)
    {
        if (_lastFragment == fragment && _lastLoadRemoteImages == _loadRemoteImages)
        {
            return;
        }
        _lastFragment = fragment;
        _lastLoadRemoteImages = _loadRemoteImages;
        var document = _model!.RenderMessageHtml(fragment, _loadRemoteImages);
        try
        {
            await EnsureCoreAsync();
            _expectingLoad = true;
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
        var plain = _model?.Reading?.Plain;
        RemoteImagesBanner.Visibility = Visibility.Collapsed;
        if (!string.IsNullOrEmpty(plain))
        {
            PlainText.Text = plain;
            ShowState(plain: true);
        }
        else
        {
            EmptyText.Text = L10n.ReadingWebviewUnavailable();
            ShowState(empty: true);
        }
    }

    private Task EnsureCoreAsync() => _coreInit ??= InitCoreAsync();

    private async Task InitCoreAsync()
    {
        await Body.EnsureCoreWebView2Async();
        var core = Body.CoreWebView2;
        var settings = core.Settings;
        // Defence in depth atop the core's sanitisation: no scripting, no host bridge.
        settings.IsScriptEnabled = false;
        settings.AreHostObjectsAllowed = false;
        settings.IsWebMessageEnabled = false;
        settings.AreDefaultContextMenusEnabled = false;
        // Block in-view navigations; allow only our NavigateToString. A clicked link opens
        // in the default browser instead (OnNavigationStarting).
        core.NavigationStarting += OnNavigationStarting;
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
    // shared-Rust launch policy (a strict scheme allowlist, http(s)/mailto only; mail is
    // hostile input) so every client is identical and consistent with what the sanitizer
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
