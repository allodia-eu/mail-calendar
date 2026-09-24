// Printing the open message (docs/reading-actions.md, "Printing a message"). The page is built in
// shared Rust; what is native is the WebView2 it is laid out in, which carries the reading host's
// gates, and the system print dialog.

using Allodia.Mailcal.Services;
using Microsoft.UI.Xaml;
using Microsoft.Web.WebView2.Core;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Views;

public sealed partial class ReadingView
{
    /// <summary>One-shot init of <c>PrintHost</c>'s gates.</summary>
    private Task? _printInit;

    /// <summary>Set just before our own NavigateToString, so the navigation lock lets it through
    /// and its completion opens the dialog.</summary>
    private bool _printExpectingLoad;

    /// <summary>The reader's remote-images choice for the page being printed.</summary>
    private bool _printLoadRemoteImages;

    /// <summary>Whether Print has a body to print: an open that finished and fetched one.
    /// Called from <c>Render</c>, which runs on every snapshot.</summary>
    private void UpdatePrintItem()
    {
        PrintItem.IsEnabled = Opened is { } opened
            && BodySnapshot is { Pending: false, LoadError: false } body
            && body.Key == opened.Key;
    }

    private async void OnPrint(object sender, RoutedEventArgs e)
    {
        if (Opened is not { } opened || BodySnapshot is not { } body || body.Key != opened.Key)
        {
            return;
        }
        ClearExportError();
        // The header as this pane draws it, under the same labels, so a printout says what the
        // screen said. Empty lines are dropped by the core.
        var document = _model!.RenderMessagePrintHtml(
            SubjectText.Text,
            [
                new PrintHeaderLine(L10n.ComposeFrom(), FromText.Text),
                new PrintHeaderLine(L10n.ComposeTo(), body.To),
                new PrintHeaderLine(L10n.ComposeCc(), body.Cc),
                new PrintHeaderLine(L10n.ComposeBcc(), body.Bcc),
                new PrintHeaderLine(L10n.QuoteSent(), DateText.Text),
            ],
            body.Html,
            body.Plain,
            _loadRemoteImages);
        try
        {
            await EnsurePrintCoreAsync();
            _printLoadRemoteImages = _loadRemoteImages;
            _printExpectingLoad = true;
            PrintHost.CoreWebView2!.NavigateToString(document);
        }
        catch (Exception ex)
        {
            // No WebView2 runtime, or the page exceeded NavigateToString's limit.
            _printExpectingLoad = false;
            Log.Warn($"reading: couldn't lay the message out to print ({ex.GetType().Name})");
            ShowPrintError(opened.Key);
        }
    }

    private Task EnsurePrintCoreAsync() => _printInit ??= InitPrintCoreAsync();

    // The reading host's gates (ReadingView.WebView.cs), on the print host.
    private async Task InitPrintCoreAsync()
    {
        await PrintHost.EnsureCoreWebView2Async();
        var core = PrintHost.CoreWebView2;
        var settings = core.Settings;
        settings.IsScriptEnabled = false;
        settings.AreHostObjectsAllowed = false;
        settings.IsWebMessageEnabled = false;
        settings.AreDefaultContextMenusEnabled = false;
        core.NavigationStarting += (_, args) =>
        {
            if (_printExpectingLoad)
            {
                return;
            }
            args.Cancel = true;
        };
        core.NavigationCompleted += (_, args) =>
        {
            if (!_printExpectingLoad)
            {
                return;
            }
            _printExpectingLoad = false;
            if (args.IsSuccess)
            {
                core.ShowPrintUI(CoreWebView2PrintDialogKind.System);
            }
            else if (Opened is { } opened)
            {
                ShowPrintError(opened.Key);
            }
        };
        core.NewWindowRequested += (_, args) => args.Handled = true;
        core.AddWebResourceRequestedFilter("*", CoreWebView2WebResourceContext.All);
        core.WebResourceRequested += (sender, args) =>
        {
            if (_printLoadRemoteImages)
            {
                return;
            }
            var uri = args.Request.Uri ?? string.Empty;
            if (uri.StartsWith("http://", StringComparison.OrdinalIgnoreCase)
                || uri.StartsWith("https://", StringComparison.OrdinalIgnoreCase))
            {
                args.Response = sender.Environment.CreateWebResourceResponse(null, 403, "Blocked", string.Empty);
            }
        };
    }

    // The export's error line, which is where this pane reports what the overflow menu could not do.
    private void ShowPrintError(string key)
    {
        _exportErrorKey = key;
        ExportError.Text = L10n.MessagePrintFailed();
        ExportError.Visibility = Visibility.Visible;
    }
}
