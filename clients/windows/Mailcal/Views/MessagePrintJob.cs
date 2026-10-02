// One print of one message through Windows' own print dialog (docs/reading-actions.md, "Printing a
// message"). The dialog is opened for the window Print was pressed in, so it stands in front of it,
// and it previews the pages this job hands it.
//
// Pages: the hidden web view lays the core's page out to PDF at the paper size the dialog has
// chosen, and Windows' PDF renderer draws each PDF page to an image. A change of paper or
// orientation in the dialog lays it out again.

using Allodia.Mailcal.Services;
using Microsoft.UI;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Media.Imaging;
using Microsoft.UI.Xaml.Printing;
using Microsoft.Web.WebView2.Core;
using Windows.Data.Pdf;
using Windows.Foundation;
using Windows.Graphics.Printing;
using Windows.Storage.Streams;

namespace Allodia.Mailcal.Views;

internal sealed class MessagePrintJob
{
    /// <summary>The resolution a page is drawn at, for the preview and the printer alike.</summary>
    private const double RenderDpi = 300;

    /// <summary>The smallest margin, in inches, whatever the printer could reach.</summary>
    private const double MinMarginInches = 0.4;

    private const double DipsPerInch = 96;

    private readonly CoreWebView2 _page;
    private readonly string _title;
    private readonly IntPtr _window;
    private readonly PrintDocument _document = new();
    private readonly IPrintDocumentSource _source;
    private readonly TaskCompletionSource<PrintTaskCompletion?> _completed =
        new(TaskCreationOptions.RunContinuationsAsynchronously);

    private List<UIElement> _sheets = [];
    private Size _laidOutFor;
    private Size _previewSize;
    private Task<List<UIElement>>? _layout;
    private Size _layoutFor;
    private PrintTask? _task;

    /// <param name="page">A web view holding the page to print, already loaded.</param>
    /// <param name="title">The job's name in the dialog and the print queue: the subject.</param>
    /// <param name="window">The window the dialog opens in front of.</param>
    public MessagePrintJob(CoreWebView2 page, string title, IntPtr window)
    {
        _page = page;
        _title = title;
        _window = window;
        // Read here, on the UI thread: the dialog asks for it from one of its own.
        _source = _document.DocumentSource;
        _document.Paginate += OnPaginate;
        _document.GetPreviewPage += OnGetPreviewPage;
        _document.AddPages += OnAddPages;
    }

    /// <summary>
    /// Shows the dialog and completes once the job has left it: submitted, cancelled or failed.
    /// <c>null</c> when the dialog did not open.
    /// </summary>
    public async Task<PrintTaskCompletion?> RunAsync()
    {
        var manager = PrintManagerInterop.GetForWindow(_window);
        manager.PrintTaskRequested += OnPrintTaskRequested;
        bool shown;
        try
        {
            shown = await PrintManagerInterop.ShowPrintUIForWindowAsync(_window);
        }
        finally
        {
            manager.PrintTaskRequested -= OnPrintTaskRequested;
        }
        if (!shown || _task is null)
        {
            return null;
        }
        return await _completed.Task;
    }

    private void OnPrintTaskRequested(PrintManager sender, PrintTaskRequestedEventArgs args)
    {
        var task = args.Request.CreatePrintTask(_title, source => source.SetSource(_source));
        task.Completed += (_, completed) => _completed.TrySetResult(completed.Completion);
        _task = task;
    }

    private void OnPaginate(object sender, PaginateEventArgs e)
    {
        var description = e.PrintTaskOptions.GetPageDescription(0);
        _previewSize = description.PageSize;
        if (_sheets.Count > 0 && _laidOutFor == description.PageSize)
        {
            _document.SetPreviewPageCount(_sheets.Count, PreviewPageCountType.Final);
            return;
        }
        // Laid out off this event, which cannot wait; the preview is asked again once it is done.
        _ = LayOutThenPreviewAsync(description);
        _document.SetPreviewPageCount(1, PreviewPageCountType.Intermediate);
    }

    private async Task LayOutThenPreviewAsync(PrintPageDescription description)
    {
        try
        {
            await LayOutAsync(description);
            _document.InvalidatePreview();
        }
        catch (Exception ex)
        {
            Log.Warn($"reading: couldn't lay the message out to print ({ex.GetType().Name})");
            _completed.TrySetResult(PrintTaskCompletion.Failed);
        }
    }

    private void OnGetPreviewPage(object sender, GetPreviewPageEventArgs e)
    {
        var index = e.PageNumber - 1;
        _document.SetPreviewPage(e.PageNumber, index < _sheets.Count ? _sheets[index] : Blank(_previewSize));
    }

    private async void OnAddPages(object sender, AddPagesEventArgs e)
    {
        try
        {
            await LayOutAsync(e.PrintTaskOptions.GetPageDescription(0));
            foreach (var sheet in _sheets)
            {
                _document.AddPage(sheet);
            }
            _document.AddPagesComplete();
        }
        catch (Exception ex)
        {
            Log.Warn($"reading: couldn't lay the message out to print ({ex.GetType().Name})");
            _document.AddPagesComplete();
            _completed.TrySetResult(PrintTaskCompletion.Failed);
        }
    }

    // One layout per paper size; a second request for the size in progress waits for the first.
    private async Task LayOutAsync(PrintPageDescription description)
    {
        var size = description.PageSize;
        if (_sheets.Count > 0 && _laidOutFor == size)
        {
            return;
        }
        if (_layout is null || _layoutFor != size)
        {
            _layoutFor = size;
            _layout = RenderAsync(description);
        }
        var sheets = await _layout;
        if (_layoutFor == size)
        {
            _sheets = sheets;
            _laidOutFor = size;
        }
    }

    private async Task<List<UIElement>> RenderAsync(PrintPageDescription description)
    {
        var size = description.PageSize;
        var printable = description.ImageableRect;
        var settings = _page.Environment.CreatePrintSettings();
        settings.Orientation = CoreWebView2PrintOrientation.Portrait;
        settings.PageWidth = size.Width / DipsPerInch;
        settings.PageHeight = size.Height / DipsPerInch;
        settings.MarginLeft = Margin(printable.Left);
        settings.MarginTop = Margin(printable.Top);
        settings.MarginRight = Margin(size.Width - printable.Right);
        settings.MarginBottom = Margin(size.Height - printable.Bottom);
        settings.ShouldPrintHeaderAndFooter = false;
        // As Edge and the other clients print: a message's own background colours stay on screen.
        settings.ShouldPrintBackgrounds = false;

        using var pdfStream = await _page.PrintToPdfStreamAsync(settings);
        var pdf = await PdfDocument.LoadFromStreamAsync(pdfStream);
        var sheets = new List<UIElement>((int)pdf.PageCount);
        var width = (uint)Math.Round(size.Width / DipsPerInch * RenderDpi);
        for (uint i = 0; i < pdf.PageCount; i++)
        {
            using var pdfPage = pdf.GetPage(i);
            using var drawn = new InMemoryRandomAccessStream();
            await pdfPage.RenderToStreamAsync(drawn, new PdfPageRenderOptions { DestinationWidth = width });
            var bitmap = new BitmapImage();
            await bitmap.SetSourceAsync(drawn);
            var sheet = Blank(size);
            sheet.Children.Add(new Image { Source = bitmap, Stretch = Stretch.Uniform });
            sheets.Add(sheet);
        }
        return sheets;
    }

    private static double Margin(double unprintableDips) =>
        Math.Max(MinMarginInches, unprintableDips / DipsPerInch);

    private static Grid Blank(Size size) => new()
    {
        Width = size.Width,
        Height = size.Height,
        Background = new SolidColorBrush(Colors.White),
    };
}
