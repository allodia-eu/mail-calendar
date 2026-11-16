// The composer is one column whose header scrolls away before its message does, the message in a
// box as tall as the column's visible part under a toolbar that stays where it is
// (docs/composer-layout.md, rules 2, 5, 6 and 7). The core decides which scroll a step moves and
// where the column scrolls; Mailcal.Tests cannot link the ScrollViewer or the WebView2, so nothing
// else sees whether the page's notices reach them.
//
// THE WHEEL OVER THE MESSAGE. The message is a page in a WebView2, which scrolls the page by every
// wheel step over it before the column sees the step, and marks the event handled. So the column
// tells the page which steps down are its own, and takes them from the handled event. The wheel is
// real input, over the message's text; the oracles are the heading at the column's top, and the
// page's own report of whether the message is scrolled, which the column carries as a signal
// because the agent cannot reach inside the page.
//
// The keys and the wheel are real, with the window in front. Nothing is sent; each composer is
// discarded.

using Allodia.Mailcal.UiTests.Driver;
using Xunit;

namespace Allodia.Mailcal.UiTests.Suites;

[Collection(ShowcaseCollection.Name)]
[Trait("Dataset", "showcase")]
public sealed class ComposerColumnTests(ShowcaseApp showcase)
{
    // Enough lines to run any window this suite meets past the bottom of the message's box.
    private const int Lines = 60;

    private const string Scrolled = "message scrolled";

    private void Case(string name, Action<App> body) =>
        showcase.Case(name, app =>
        {
            new DraftsFlow(app).OpenNewComposer();
            body(app);
        }, app => app.CloseComposer());

    private static Element Editor(App app) =>
        app.Agent.Find(Filter.ById("Editor")) ?? throw new InvalidOperationException("the composer draws no editor");

    private static Element Column(App app) =>
        app.Agent.Find(Filter.ById("ComposerScroll")) ?? throw new InvalidOperationException("the composer draws no column");

    private static Element? Title(App app) => app.Agent.Find(Filter.ById("ComposerTitle"));

    /// <summary>Whether the page last reported the message scrolled inside its box.</summary>
    private static bool MessageScrolled(App app) => app.Agent.Find(Filter.ById("ComposerScroll") with { Signal = Scrolled }) is not null;

    /// <summary>Puts the caret in the message and types <see cref="Lines"/> lines into it, at the
    /// very end of it, past the signature a new message opens with.</summary>
    private static void WriteALongMessage(App app)
    {
        var editor = app.Steady(Editor(app));
        Input.Click(editor);
        app.Agent.Settle(quiet: TimeSpan.FromMilliseconds(300), floor: TimeSpan.FromMilliseconds(300));
        Input.Chord(Input.End, Input.Control);
        for (var line = 0; line < Lines; line++)
        {
            Input.Type($"line {line}");
            Input.Chord(Input.Enter);
            Thread.Sleep(20);
        }
        app.Agent.Settle(quiet: TimeSpan.FromMilliseconds(500), floor: TimeSpan.FromMilliseconds(800));
    }

    /// <summary>The middle of the message's visible part, in front.</summary>
    private static (int X, int Y) OverTheMessage(App app)
    {
        var editor = app.Steady(Editor(app));
        Input.BringToFront(editor.Window);
        return ((int)(editor.X + editor.Width / 2), (int)(editor.Y + editor.Height / 2));
    }

    [OfflineFact]
    public void TypingPastTheBottomCarriesTheHeaderAwayAndTheMessageScrollsUnderIt() => Case(nameof(TypingPastTheBottomCarriesTheHeaderAwayAndTheMessageScrollsUnderIt), app =>
    {
        Assert.True(Title(app) is { Rendered: true }, "the composer opened scrolled away from its own heading");
        WriteALongMessage(app);
        Assert.True(Title(app) is not { Rendered: true },
            "the heading is still on screen after the message ran past the bottom: the column did not follow the caret, so the line being typed is out of sight");
        // The agent gives the editor the rectangle of its visible part. At the column's end the
        // box's bottom is the column's: nothing is under the message, and the box is not cut by
        // the edge. (The column's own rectangle is its content's, so only its bottom is compared.)
        var (column, editor) = (app.Steady(Column(app)), app.Steady(Editor(app)));
        Assert.True(Math.Abs(editor.Y + editor.Height - (column.Y + column.Height)) <= 2,
            $"the message's box ends at {editor.Y + editor.Height} and the column at {column.Y + column.Height}: something is under the message, or the column stopped short of its end");
        Assert.True(App.Eventually(() => MessageScrolled(app), App.Patience),
            "sixty lines in, the page never reported the message scrolled inside its box: the scroll notice did not reach the column, or the page grew instead of scrolling");
    });

    [OfflineFact]
    public void UpTheMessageGoesBackToItsTopBeforeTheHeaderReturns() => Case(nameof(UpTheMessageGoesBackToItsTopBeforeTheHeaderReturns), app =>
    {
        WriteALongMessage(app);
        Assert.True(App.Eventually(() => MessageScrolled(app), App.Patience), "the long message is not scrolled, so this case cannot see which scroll goes back first");
        var (x, y) = OverTheMessage(app);
        for (var notch = 0; notch < 80 && Title(app) is not { Rendered: true }; notch++)
        {
            Input.Wheel(x, y, 1);
            Thread.Sleep(60);
            Assert.False(Title(app) is { Rendered: true } && MessageScrolled(app),
                "the heading came back while the message was still scrolled: the column took a step up that was the message's");
        }
        Assert.True(App.Eventually(() => Title(app) is { Rendered: true }, TimeSpan.FromSeconds(3)),
            "eighty notches of the wheel over the message did not bring the heading back: the column never saw the wheel once the message was at its top");
    });

    [OfflineFact]
    public void DownTheHeaderGoesBeforeTheMessageScrolls() => Case(nameof(DownTheHeaderGoesBeforeTheMessageScrolls), app =>
    {
        WriteALongMessage(app);
        // At the column's end the message's box fills its visible part, which is the most of the
        // box the agent can show.
        var whole = app.Steady(Editor(app)).Height;
        var (x, y) = OverTheMessage(app);
        for (var notch = 0; notch < 80 && Title(app) is not { Rendered: true }; notch++)
        {
            Input.Wheel(x, y, 1);
            Thread.Sleep(60);
        }
        Assert.True(App.Eventually(() => Title(app) is { Rendered: true } && !MessageScrolled(app), TimeSpan.FromSeconds(3)),
            "the wheel did not bring the composer back to its top, so this case cannot start from there");
        (x, y) = OverTheMessage(app);
        for (var notch = 0; notch < 80 && !MessageScrolled(app); notch++)
        {
            Input.Wheel(x, y, -1);
            Thread.Sleep(60);
        }
        Assert.True(App.Eventually(() => MessageScrolled(app), TimeSpan.FromSeconds(3)),
            "eighty notches down over a long message never scrolled it: the page held every step, or the column kept them");
        // The heading alone is no oracle: one step carries it away. The box is wholly in view only
        // once the column is at its end.
        var shown = app.Steady(Editor(app)).Height;
        Assert.True(shown >= whole - 2,
            $"the message scrolled while {whole - shown}px of its box were still under the column's edge: the page took a step down the column should have carried, so both moved");
    });
}
