// The composer is one column that scrolls as a whole, the message grows with its text, and its
// toolbar stays in view (docs/composer-layout.md, rules 2, 5, 6 and 7). The core decides the editor's height and where the
// column scrolls; Mailcal.Tests cannot link the ScrollViewer or the WebView2, so nothing else sees
// whether the page's notices reach them.
//
// THE WHEEL OVER THE MESSAGE. The message is a page in a WebView2, which marks every wheel event
// over it handled. In growing mode the page has nothing to scroll, and a wheel that stopped there
// would leave the column reachable only by its scroll bar, the one gesture people use most gone
// over most of the composer. So the wheel is real input, over the message's text, and the oracle
// is the heading at the column's top coming back.
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

    /// <summary>Puts the caret in the message and types <see cref="Lines"/> lines into it; at the
    /// very end of it when <paramref name="atTheEnd"/>, past the signature a new message opens
    /// with.</summary>
    private static void WriteALongMessage(App app, bool atTheEnd = false)
    {
        var editor = app.Steady(Editor(app));
        Input.Click(editor);
        app.Agent.Settle(quiet: TimeSpan.FromMilliseconds(300), floor: TimeSpan.FromMilliseconds(300));
        if (atTheEnd)
        {
            Input.Chord(Input.End, Input.Control);
        }
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
    public void TheLineBeingTypedIsWhollyInViewWithRoomUnderIt() => Case(nameof(TheLineBeingTypedIsWhollyInViewWithRoomUnderIt), app =>
    {
        WriteALongMessage(app, atTheEnd: true);
        // The caret is on the message's last line, so following it takes the column to its end,
        // where the column's bottom padding shows under the editor. The agent gives the editor the
        // rectangle of its visible part: an editor that runs on under the column's edge ends there.
        // A line typed past the end arrives before the column has grown to hold it, which is the
        // case a scroll that is not kept and asked again stops short on.
        var column = app.Agent.Find(Filter.ById("ComposerScroll")) ?? throw new InvalidOperationException("the composer draws no column");
        var editor = app.Steady(Editor(app));
        var under = column.Y + column.Height - (editor.Y + editor.Height);
        Assert.True(under >= 8,
            $"the editor runs to {under}px from the column's bottom edge after the last line was typed: the column stopped short of its end, so the line being typed is cut by the edge");
    });

    [OfflineFact]
    public void TheToolbarStaysInViewInALongMessage() => Case(nameof(TheToolbarStaysInViewInALongMessage), app =>
    {
        WriteALongMessage(app);
        Assert.True(Title(app) is not { Rendered: true }, "the long message left the heading on screen, so this case cannot see the toolbar kept in view");
        // The toolbar is inside the page, where the agent cannot reach: the column carries the
        // page's own answer to where it drew it (ComposerView.Layout.cs).
        Assert.True(app.Agent.Wait(Filter.ById("ComposerScroll") with { Signal = "toolbar kept in view" }, App.Patience) is not null,
            "the formatting toolbar scrolled away with the top of the message: the page was not told where the column shows it from, or drew the toolbar in its place");
    });

    [OfflineFact]
    public void TheWheelOverTheMessageScrollsTheColumn() => Case(nameof(TheWheelOverTheMessageScrollsTheColumn), app =>
    {
        WriteALongMessage(app);
        Assert.True(Title(app) is not { Rendered: true }, "the long message left the heading on screen, so this case cannot see the wheel move anything");
        // The agent gives the editor the rectangle of its visible part, which the message fills once
        // the heading and the toolbar have scrolled off the top, so its middle is over the text.
        var editor = app.Steady(Editor(app));
        var (x, y) = ((int)(editor.X + editor.Width / 2), (int)(editor.Y + editor.Height / 2));
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
