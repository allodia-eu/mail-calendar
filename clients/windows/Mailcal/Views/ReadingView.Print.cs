// Printing the open message (docs/reading-actions.md, "Printing a message"). The page is built in
// shared Rust; what is native is the WebView2 it is laid out in, which carries the reading host's
// gates, and Windows' print dialog, which MessagePrintJob opens for this view's window.

using Allodia.Mailcal.Services;
using Microsoft.UI;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Automation.Peers;
using Microsoft.UI.Xaml.Controls;
using Microsoft.Web.WebView2.Core;
using uniffi.mailcal_bindings;
using Windows.Graphics.Printing;

namespace Allodia.Mailcal.Views;

public sealed partial class ReadingView
{
    /// <summary>Windows with a print dialog up. A window has one dialog at a time, and a press
    /// while it is up does nothing: the dialog is in front of the window that would take it.</summary>
    private static readonly HashSet<IntPtr> WindowsPrinting = [];

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
        if (Opened is not { } opened || BodySnapshot is not { } body || body.Key != opened.Key
            || XamlRoot is null)
        {
            return;
        }
        var window = Win32Interop.GetWindowFromWindowId(XamlRoot.ContentIslandEnvironment.AppWindowId);
        if (!WindowsPrinting.Add(window))
        {
            return;
        }
        ClearExportError();
        var subject = SubjectText.Text;
        // The header as this pane draws it, under the same labels, so a printout says what the
        // screen said. Empty lines are dropped by the core.
        var document = _model!.RenderMessagePrintHtml(
            subject,
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
            var loaded = HardenPrintPage(page.CoreWebView2, _loadRemoteImages);
            page.CoreWebView2.NavigateToString(document);
            if (!await loaded)
            {
                throw new InvalidOperationException("the print page did not load");
            }
            var outcome = await new MessagePrintJob(page.CoreWebView2, subject, window).RunAsync();
            if (outcome == PrintTaskCompletion.Failed)
            {
                ShowPrintError(opened.Key);
            }
        }
        catch (Exception ex)
        {
            // No WebView2 runtime, a page over NavigateToString's limit, or no print dialog.
            Log.Warn($"reading: couldn't print the message ({ex.GetType().Name})");
            ShowPrintError(opened.Key);
        }
        finally
        {
            WindowsPrinting.Remove(window);
            PrintPages.Children.Remove(page);
            page.Close();
        }
    }

    // A WebView2 starts only inside the visual tree, so the page is added to PrintPages, which is
    // transparent and one pixel square; the job lays the page out at the paper's size.
    private WebView2 AddPrintPage()
    {
        var page = new WebView2 { Width = 1, Height = 1, IsTabStop = false };
        AutomationProperties.SetAccessibilityView(page, AccessibilityView.Raw);
        PrintPages.Children.Add(page);
        return page;
    }

    // The reading host's gates (ReadingView.WebView.cs), on one print page, which loads the one
    // document it was made for and then nothing. The remote-images choice is the one this page was
    // built with, so a later opt-in on the pane cannot reach it. Completes with whether it loaded.
    private static Task<bool> HardenPrintPage(CoreWebView2 core, bool loadRemoteImages)
    {
        var loaded = new TaskCompletionSource<bool>();
        var settings = core.Settings;
        settings.IsScriptEnabled = false;
        settings.AreHostObjectsAllowed = false;
        settings.IsWebMessageEnabled = false;
        settings.AreDefaultContextMenusEnabled = false;
        core.NavigationStarting += (_, args) =>
        {
            if (loaded.Task.IsCompleted)
            {
                args.Cancel = true;
            }
        };
        core.NavigationCompleted += (_, args) => loaded.TrySetResult(args.IsSuccess);
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
        return loaded.Task;
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
