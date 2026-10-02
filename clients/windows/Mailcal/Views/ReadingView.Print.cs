// Printing the open message (docs/reading-actions.md, "Printing a message"). The page is built in
// shared Rust; what is native is the WebView2 it is laid out in, which carries the reading host's
// gates, and the system print dialog.

using Allodia.Mailcal.Services;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Automation.Peers;
using Microsoft.UI.Xaml.Controls;
using Microsoft.Web.WebView2.Core;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Views;

public sealed partial class ReadingView
{
    /// <summary>How many print pages stay alive, oldest dropped first.</summary>
    /// <remarks>
    /// Each print gets a web view of its own, because the system dialog prints the page its web
    /// view holds when the reader presses Print in it, not the page it opened on: a second print
    /// through a shared web view replaced the first dialog's page and that dialog then failed.
    /// <c>ShowPrintUI</c> reports neither the dialog closing nor the job leaving, so a page cannot
    /// be released when it is done; it is released when this many newer prints have started.
    /// </remarks>
    private const int PrintPagesKept = 4;

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
        var page = AddPrintPage();
        try
        {
            await page.EnsureCoreWebView2Async();
            HardenPrintPage(page.CoreWebView2, _loadRemoteImages, opened.Key);
            page.CoreWebView2.NavigateToString(document);
        }
        catch (Exception ex)
        {
            // No WebView2 runtime, or the page exceeded NavigateToString's limit.
            Log.Warn($"reading: couldn't lay the message out to print ({ex.GetType().Name})");
            ShowPrintError(opened.Key);
        }
    }

    // A WebView2 starts only inside the visual tree, so the page is added to PrintPages, which is
    // transparent and one pixel square; the dialog lays the page out at the paper's size.
    private WebView2 AddPrintPage()
    {
        var page = new WebView2 { Width = 1, Height = 1, IsTabStop = false };
        AutomationProperties.SetAccessibilityView(page, AccessibilityView.Raw);
        PrintPages.Children.Add(page);
        while (PrintPages.Children.Count > PrintPagesKept)
        {
            var oldest = (WebView2)PrintPages.Children[0];
            PrintPages.Children.RemoveAt(0);
            oldest.Close();
        }
        return page;
    }

    // The reading host's gates (ReadingView.WebView.cs), on one print page, which loads the one
    // document it was made for and then nothing. The remote-images choice is the one this page was
    // built with, so a later opt-in on the pane cannot reach it.
    private void HardenPrintPage(CoreWebView2 core, bool loadRemoteImages, string key)
    {
        var expectingLoad = true;
        var settings = core.Settings;
        settings.IsScriptEnabled = false;
        settings.AreHostObjectsAllowed = false;
        settings.IsWebMessageEnabled = false;
        settings.AreDefaultContextMenusEnabled = false;
        core.NavigationStarting += (_, args) =>
        {
            if (!expectingLoad)
            {
                args.Cancel = true;
            }
        };
        core.NavigationCompleted += (_, args) =>
        {
            if (!expectingLoad)
            {
                return;
            }
            expectingLoad = false;
            if (args.IsSuccess)
            {
                core.ShowPrintUI(CoreWebView2PrintDialogKind.System);
            }
            else
            {
                ShowPrintError(key);
            }
        };
        core.NewWindowRequested += (_, args) => args.Handled = true;
        core.AddWebResourceRequestedFilter("*", CoreWebView2WebResourceContext.All);
        core.WebResourceRequested += (sender, args) =>
        {
            if (loadRemoteImages)
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

    private void ClosePrintPages()
    {
        foreach (var page in PrintPages.Children.OfType<WebView2>())
        {
            page.Close();
        }
        PrintPages.Children.Clear();
    }

    // The export's error line, which is where this pane reports what the overflow menu could not do.
    // Only while the message it is about is still the one open.
    private void ShowPrintError(string key)
    {
        if (Opened?.Key != key)
        {
            return;
        }
        _exportErrorKey = key;
        ExportError.Text = L10n.MessagePrintFailed();
        ExportError.Visibility = Visibility.Visible;
    }
}
