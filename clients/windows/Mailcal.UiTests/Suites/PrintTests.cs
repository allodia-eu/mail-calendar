// Print, from the reading pane's overflow menu (docs/reading-actions.md, "Printing a message").
//
// WHAT ONLY THIS SUITE CAN SEE. The page is built and unit-tested in `mailcal-app`; what is native
// here is the hidden WebView2 that lays it out and Windows' print dialog, opened for this window
// through PrintManager. Three properties of that dialog are invisible to an assembly without WinUI:
//
//   * It belongs to the window Print was pressed in. A dialog that does not (WebView2's own system
//     dialog is a window of the web view's process) can open BEHIND the app, and every later press
//     then does nothing, because a window has one print dialog at a time.
//   * It previews the pages this client hands it, rather than "This app doesn't support print
//     preview".
//   * The job is named by the subject, and prints the message it was opened for.
//
// HOW IT READS A PRINT. Through "Microsoft Print to PDF", into a file, never a real printer. The
// PDF's /Title is the job's name, and it is the one thing in the file that says WHICH message was
// printed. The harness because the body has to have arrived for Print to be offered, and the seeded
// subjects are stable strings. Nothing here writes mail.

using Allodia.Mailcal.UiTests.Driver;
using Xunit;

namespace Allodia.Mailcal.UiTests.Suites;

[Collection(HarnessCollection.Name)]
public sealed class PrintTests(HarnessApp harness)
{
    private const string FirstSubject = "Harness baseline message";
    private const string SecondSubject = "Fixed-width newsletter that cannot reflow";

    /// <summary>Opens the overflow and returns its Print item, once the body has arrived and made it
    /// enabled.</summary>
    private static Element PrintItem(App app)
    {
        // The action row arrives with the body, after the subject.
        app.Agent.Invoke(app.Agent.Wait(Filter.ById("ReadingOverflow") with { Window = app.Mailbox.Hwnd }, App.Patience)
            ?? throw new InvalidOperationException("the reading pane has no overflow menu"));
        var item = app.Agent.Wait(Filter.ById("ReadingPrint"), TimeSpan.FromSeconds(10))
            ?? throw new InvalidOperationException("the overflow offered no Print item");
        Assert.True(App.Eventually(() => app.Agent.Read(item).Element.Enabled, TimeSpan.FromSeconds(10)),
            "Print must be offered once the body has arrived (docs/reading-actions.md)");
        return app.Agent.Read(item).Element;
    }

    private static void CloseMenu(App app)
    {
        if (app.Agent.Find(Filter.ById("ReadingPrint")) is null)
        {
            return;
        }
        Input.BringToFront(app.Mailbox.Hwnd);
        Input.Press(Input.Escape);
        app.Agent.WaitGone(Filter.ById("ReadingPrint"), TimeSpan.FromSeconds(5));
    }

    [HarnessFact]
    public void PrintIsOfferedBelowTheExportOnceTheBodyHasArrived() => harness.Case(nameof(PrintIsOfferedBelowTheExportOnceTheBodyHasArrived), app =>
    {
        app.OpenMessage(FirstSubject);
        var print = PrintItem(app);
        var export = app.Agent.Find(Filter.ById("ReadingExportEml")) ?? throw new InvalidOperationException("the overflow offers no Save as .eml");
        Assert.True(print.Y > export.Y, "Print sits below Save as .eml in the overflow, on every platform");
    }, CloseMenu);

    [HarnessFact]
    public void TheDialogBelongsToThisWindowPreviewsThePageAndIsNamedByTheSubject() =>
        harness.Case(nameof(TheDialogBelongsToThisWindowPreviewsThePageAndIsNamedByTheSubject), app =>
        {
            app.OpenMessage(SecondSubject);
            app.Agent.Invoke(PrintItem(app));
            var dialog = PrintDialogs.Wait();
            Assert.False(PrintDialogs.Enabled(app.Mailbox.Hwnd),
                "the print dialog must hold the window it was opened from, which is what keeps it in front of the app; the mailbox still takes input, so the dialog is not its own (docs/reading-actions.md)");
            Assert.True(dialog.Current.Name.StartsWith(SecondSubject, StringComparison.Ordinal),
                $"the dialog is titled by the job's name, which must be the subject; it read '{dialog.Current.Name}'");
            // The dialog says so in this element when the app hands it no preview pages.
            var noPreview = Desktop.ById("NoPreviewAvailableText", dialog);
            Assert.True(noPreview is null, $"the dialog shows no preview: '{noPreview?.Current.Name}'");
            Desktop.Press(Desktop.ById("CloseButton", dialog) ?? throw new InvalidOperationException("the print dialog has no Cancel"));
            Assert.True(App.Eventually(() => PrintDialogs.Enabled(app.Mailbox.Hwnd), TimeSpan.FromSeconds(5)),
                "the window must take input again once the dialog is cancelled");
            var error = app.Agent.Find(Filter.ById("ReadingExportError") with { OnScreen = true });
            Assert.True(error is not { Rendered: true }, $"a cancelled print is the reader's choice, not a failure; the pane said '{error?.Name}'");
        }, app => PrintDialogs.CloseAll(app.ProcessId));

    [HarnessFact]
    public void TheJobPrintsTheMessageItWasOpenedFor()
    {
        var destination = Path.Combine(Path.GetTempPath(), $"mailcal-uitest-print-{Guid.NewGuid():N}.pdf");
        harness.Case(nameof(TheJobPrintsTheMessageItWasOpenedFor), app =>
        {
            app.OpenMessage(FirstSubject);
            app.Agent.Invoke(PrintItem(app));
            Desktop.Press(PrintDialogs.ChoosePdfPrinter(PrintDialogs.Wait()));
            SaveDialogs.Save(app.ProcessId, destination);
            Assert.True(PrintDialogs.PdfTitle(destination) == FirstSubject,
                "the printed job must be the message Print was pressed on, named by its subject (docs/reading-actions.md)");
        }, app =>
        {
            PrintDialogs.CloseAll(app.ProcessId);
            File.Delete(destination);
        });
    }
}
