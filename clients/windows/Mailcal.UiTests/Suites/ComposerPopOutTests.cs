// Popping the reading pane's composer out into a window of its own (docs/reading-window.md,
// "Popping a composer out"): the window holds exactly what the pane's composer held, the pane comes
// back, and the two views are one draft, so closing the window leaves one copy in Drafts.
//
// WHY HERE. Every rule below is wiring in ComposerView.PopOut.cs and MainWindow.Compose.cs, which
// link WinUI: the state read off the pane, the window seeded from it rather than from the origin,
// and the pane's teardown leaving alone the composition the window now owns. That last one fails
// silently anywhere else: the window still opens and still saves, only beside a draft rather than
// over it.
//
// WHY THE HARNESS. A draft is a mail write, and the showcase engine performs none. The copies are
// counted on the server, never from the list, whose row can stand for a whole conversation; each
// case takes its draft off the server again.
//
// The body is the editor's own WebView2, in the runtime's process, so it is read through UI
// Automation, as ComposerUndoTests reads it.

using System.Diagnostics;
using System.Windows.Automation;
using Allodia.Mailcal.UiTests.Driver;
using Xunit;

namespace Allodia.Mailcal.UiTests.Suites;

[Collection(HarnessCollection.Name)]
[Trait("Dataset", "harness")]
public sealed class ComposerPopOutTests(HarnessApp harness)
{
    private const string Written = "Written before the pop-out";

    private const string Original = "Harness baseline message";

    /// <summary>A line of the original's own text, which only its quote can bring into a reply.</summary>
    private const string OriginalLine = "Baseline plaintext message with a unique Message-ID.";

    private static string Subject(string what) => $"Pop-out suite {DraftsFlow.Run} {what}";

    private void Case(string name, string subject, Action<App, DraftsFlow> body) => harness.Case(
        name,
        app => body(app, new DraftsFlow(app)),
        app =>
        {
            app.CloseExtraWindows();
            app.CloseComposer();
            Harness.RemoveMail(Harness.Alice, subject);
        });

    [HarnessFact]
    public void ANewMessagePoppedOutOpensAWindowHoldingWhatWasWritten() => Case(nameof(ANewMessagePoppedOutOpensAWindowHoldingWhatWasWritten), Subject("new"), (app, flow) =>
    {
        var subject = Subject("new");
        flow.OpenNewComposer();
        flow.SetText("ToField", Harness.Bob);
        flow.SetText("SubjectBox", subject);
        TypeInBody(app, Written);
        var window = PopOut(app, subject);

        var pill = app.Agent.Find(new Filter { Type = "Button", NameLike = $"{Harness.Bob}, *", Window = window.Hwnd });
        Assert.True(pill is not null, $"the window does not hold the recipient as a pill ({Harness.Bob})");
        var box = app.Agent.Find(Filter.ById("SubjectBox") with { Window = window.Hwnd });
        Assert.True(subject == (box is null ? null : app.Agent.Read(box).Value), "the window does not hold the subject the pane's composer had");
        Assert.True(App.Eventually(() => BodyText().Contains(Written, StringComparison.Ordinal), TimeSpan.FromSeconds(10)),
            $"the window's body does not hold what was written in the pane; it reads '{BodyText()}'");
        Assert.True(app.Agent.Find(Filter.ById("PopOutButton") with { Window = window.Hwnd, OnScreen = true }) is null,
            "the window offers to pop itself out: a composer already in a window has nowhere further to go");
        Assert.True(app.Agent.Find(Filter.ById("SendButton") with { Window = app.Mailbox.Hwnd }) is null,
            "the pane still holds a composer after the pop-out: the column belongs to the reading pane again");
    });

    [HarnessFact]
    public void ClosingThePoppedOutWindowLeavesOneCopyInDrafts() => Case(nameof(ClosingThePoppedOutWindowLeavesOneCopyInDrafts), Subject("one copy"), (app, flow) =>
    {
        // Saved in the pane first, so a window that saved beside that copy rather than over it, or
        // a pane teardown that forgot the composition the window now saves under, leaves two.
        var subject = Subject("one copy");
        flow.NewSavedDraft(subject);
        TypeInBody(app, Written);
        PopOut(app, subject);
        app.CloseExtraWindows();
        Assert.True(flow.WaitDraftRow(subject), "closing the popped-out window left no draft in Drafts");
        // A second copy can land after the first, so the server is watched for one rather than
        // asked once.
        var copies = 0;
        Assert.False(App.Eventually(() => (copies = Harness.CountMail(Harness.Alice, subject)) > 1, TimeSpan.FromSeconds(8)),
            $"the server holds {copies} copies of the draft: the pane and its window are one draft, so it must hold one");
        Assert.True(copies == 1, $"the server holds {copies} copies of the draft, not the one closing the window kept");
    });

    [HarnessFact]
    public void AReplyPoppedOutStaysAReplyWithItsQuote() => Case(nameof(AReplyPoppedOutStaysAReplyWithItsQuote), Subject("reply"), (app, flow) =>
    {
        var subject = Subject("reply");
        app.OpenFolder("Inbox");
        app.Agent.Invoke(app.RowTitled(Original));
        app.Agent.Invoke(app.Agent.Wait(Filter.ById("ReadingReply") with { OnScreen = true }, App.Patience)
            ?? throw new InvalidOperationException("the reading pane never offered Reply"));
        if (app.Agent.Wait(Filter.ById("SendButton"), App.Patience) is null)
        {
            throw new InvalidOperationException("no composer within 30 s of pressing Reply");
        }
        flow.WaitEditorSeeded();
        flow.SetText("SubjectBox", subject);
        TypeInBody(app, Written);
        var window = PopOut(app, subject);

        Assert.True(DraftsFlow.Reply == app.Agent.Find(Filter.ById("ComposerTitle") with { Window = window.Hwnd })?.Name,
            "the popped-out reply is titled as something else: it left the pane a reply and must arrive one");
        Assert.True(App.Eventually(() => BodyText().Contains(OriginalLine, StringComparison.Ordinal), TimeSpan.FromSeconds(10)),
            $"the popped-out reply lost its quote; the body reads '{BodyText()}'");
        Assert.Contains(Written, BodyText(), StringComparison.Ordinal);
    });

    /// <summary>Presses the pane composer's pop-out and waits for the window, which is named after
    /// the draft's subject.</summary>
    private static AppWindow PopOut(App app, string subject)
    {
        var button = app.Agent.Find(Filter.ById("PopOutButton", "Button") with { Window = app.Mailbox.Hwnd, OnScreen = true })
            ?? throw new InvalidOperationException("the reading pane's composer draws no pop-out button");
        app.Agent.Invoke(button);
        var window = app.WaitWindow(subject, timeoutSec: 30);
        if (app.Agent.Wait(Filter.ById("Editor") with { Window = window.Hwnd, Signal = "seeded" }, App.Patience) is null)
        {
            throw new InvalidOperationException("the popped-out composer's editor did not finish seeding within 30 s");
        }
        Assert.True(app.Agent.WaitGone(Filter.ById("SendButton") with { Window = app.Mailbox.Hwnd }, TimeSpan.FromSeconds(10)),
            "the pane's composer is still there after the window opened");
        return window;
    }

    /// <summary>Types at the end of the body's first line, the way a person does.</summary>
    private static void TypeInBody(App app, string text)
    {
        var editor = app.Agent.Find(Filter.ById("Editor") with { Window = app.Mailbox.Hwnd })
            ?? throw new InvalidOperationException("the composer has no editor");
        Input.BringToFront(editor.Window);
        Input.ClickAt((int)(editor.X + 60), (int)(editor.Y + 30));
        // The page takes focus a moment after the click; a key sent before it is lost.
        Thread.Sleep(1200);
        Input.Type(text);
        app.Agent.Settle();
    }

    /// <summary>The WebView2 runtime's own top-level windows, where the editor's page is.</summary>
    private static IEnumerable<AutomationElement> BrowserWindows() =>
        AutomationElement.RootElement.FindAll(TreeScope.Children, Condition.TrueCondition).Cast<AutomationElement>()
            .Where(w => w.Current.ClassName.StartsWith("Chrome_WidgetWin", StringComparison.Ordinal) && ProcessName(w.Current.ProcessId) == "msedgewebview2");

    private static string ProcessName(int id)
    {
        try
        {
            using var process = Process.GetProcessById(id);
            return process.ProcessName;
        }
        catch (ArgumentException)
        {
            return "";
        }
    }

    /// <summary>The text of every editor body open, joined: after a pop-out the window's is the
    /// only one, the pane's having gone with its composer.</summary>
    private static string BodyText()
    {
        var named = new PropertyCondition(AutomationElement.NameProperty, Catalog.English("editor_placeholder"));
        return string.Join("\n", BrowserWindows()
            .Select(w => w.FindFirst(TreeScope.Descendants, named))
            .Where(body => body is not null)
            .Select(body => ((TextPattern)body.GetCurrentPattern(TextPattern.Pattern)).DocumentRange.GetText(-1)));
    }
}
