// The composer is one column that scrolls as a whole, and the message grows with its text
// (docs/composer-layout.md, rules 2, 5 and 6). The core decides the editor's height and where the
// column scrolls; Mailcal.Tests cannot link the ScrollViewer or the WebView2, so nothing else sees
// whether the page's notices reach them.
//
// THE WHEEL OVER THE MESSAGE. The message is a page in a WebView2, which takes the wheel for its
// own scroll. In growing mode the page has nothing to scroll, and a wheel that stopped there would
// leave the column reachable only by its scroll bar, the one gesture people use most gone over most
// of the composer. So the wheel is real input, over the message's text, and the oracle is the
// heading at the column's top moving.
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
    // Enough lines to run any window this suite meets past the bottom of the column.
    private const int Lines = 60;

    private void Case(string name, Action<App> body) =>
        showcase.Case(name, app =>
        {
            new DraftsFlow(app).OpenNewComposer();
            body(app);
        }, app => app.CloseComposer());

    private static Element Editor(App app) =>
        app.Agent.Find(Filter.ById("Editor")) ?? throw new InvalidOperationException("the composer draws no editor");

    private static Element? Title(App app) => app.Agent.Find(Filter.ById("ComposerTitle"));

    /// <summary>Puts the caret in the message and types <see cref="Lines"/> lines into it.</summary>
    private static void WriteALongMessage(App app)
    {
        var editor = app.Steady(Editor(app));
        Input.Click(editor);
        app.Agent.Settle(quiet: TimeSpan.FromMilliseconds(300), floor: TimeSpan.FromMilliseconds(300));
        for (var line = 0; line < Lines; line++)
        {
            Input.Type($"line {line}");
            Input.Chord(Input.Enter);
            Thread.Sleep(20);
        }
        app.Agent.Settle(quiet: TimeSpan.FromMilliseconds(500), floor: TimeSpan.FromMilliseconds(800));
    }

    [OfflineFact]
    public void TypingPastTheBottomGrowsTheMessageAndTheColumnFollowsTheCaret() => Case(nameof(TypingPastTheBottomGrowsTheMessageAndTheColumnFollowsTheCaret), app =>
    {
        var before = app.Steady(Editor(app));
        Assert.True(Title(app) is { Rendered: true }, "the composer opened scrolled away from its own heading");
        WriteALongMessage(app);
        var after = app.Steady(Editor(app));
        Assert.True(after.Height > before.Height,
            $"the editor is {after.Height}px tall after {Lines} lines, as it was before ({before.Height}px): the page's height notice never reached it, so the message scrolls inside itself");
        Assert.True(Title(app) is not { Rendered: true },
            "the heading is still on screen after the message ran past the bottom: the column did not follow the caret, so the line being typed is out of sight");
    });

    [OfflineFact]
    public void TheWheelOverTheMessageScrollsTheColumn() => Case(nameof(TheWheelOverTheMessageScrollsTheColumn), app =>
    {
        WriteALongMessage(app);
        Assert.True(Title(app) is not { Rendered: true }, "the long message left the heading on screen, so this case cannot see the wheel move anything");
        var editor = app.Steady(Editor(app));
        var column = app.Steady(app.Agent.Find(Filter.ById("ComposerScroll")) ?? throw new InvalidOperationException("the composer has no column scroller"));
        // The middle of the column's visible part, which the message fills once the heading and
        // its toolbar have scrolled off the top: the pointer is over the message's text.
        var (x, y) = ((int)(column.X + column.Width / 2), (int)(column.Y + column.Height / 2));
        Assert.True(y > editor.Y && y < editor.Y + editor.Height, "the middle of the column is not over the message, so the wheel would not be turned where this case says");
        Input.BringToFront(editor.Window);
        for (var notch = 0; notch < 40 && Title(app) is not { Rendered: true }; notch++)
        {
            Input.Wheel(x, y, 1);
            Thread.Sleep(60);
        }
        Assert.True(App.Eventually(() => Title(app) is { Rendered: true }, TimeSpan.FromSeconds(3)),
            "forty notches of the wheel over the message did not bring the heading back: the editor's page took the wheel and the column never saw it");
    });
}
