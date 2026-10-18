// A message in a window of its own (docs/reading-window.md): the double-click that opens it, the
// pane it must not disturb, the action row it carries, and the composer window a reply raises.
//
// WHY HERE AND NOT IN Mailcal.Tests. Everything here is a second top-level window, which a plain
// net10.0 assembly cannot see, and the rules that matter (whether the pane moved, whether a second
// double-click stacked a window or raised the open one) are the ones a green unit test would not
// notice. The pure halves are pinned in Mailcal.Tests/ReadingWindowsTests.cs.
//
// WHY THE HARNESS. The window opens a real body into a slot of its own; the showcase engine
// performs no real read. WHY REAL INPUT. A double-click is the gesture under test, and no
// automation pattern raises one: the point is what the second press does. The context-menu route
// needs none, which is why it is here too: it is the route a keyboard or a screen reader takes.
//
// The log at debug, because one case reads it: opening a window must not activate the mailbox, and
// nothing outside the app sees a steal that is over in tens of milliseconds.
//
// NOT HERE: archive and delete closing the window. Proving it spends a seeded message on the shared
// harness, and nothing puts one back, so it is verified by hand (docs/reading-window.md).

using System.Globalization;
using System.Text.RegularExpressions;
using Allodia.Mailcal.UiTests.Driver;
using Xunit;
using static Allodia.Mailcal.UiTests.Suites.ReadingWindowFlow;

namespace Allodia.Mailcal.UiTests.Suites;

[Collection(DebugLogHarnessCollection.Name)]
public sealed class ReadingWindowTests(DebugLogHarnessApp harness)
{
    private void Case(string name, Action<App, ReadingWindowFlow> body) =>
        harness.Case(name, app => body(app, new ReadingWindowFlow(app)), app => app.CloseExtraWindows());

    [HarnessFact]
    public void ADoubleClickOpensTheMessageInAWindowOfItsOwn() => Case(nameof(ADoubleClickOpensTheMessageInAWindowOfItsOwn), (app, _) =>
    {
        app.RowClicks(OtherSubject);
        app.RowClicks(RowSubject, times: 2);
        app.WaitWindow(RowSubject);
        Assert.True(app.ExtraWindows().Count == 1, "exactly one window opened");
    });

    [HarnessFact]
    public void AndLeavesTheReadingPaneOnTheMessageItWasShowing() => Case(nameof(AndLeavesTheReadingPaneOnTheMessageItWasShowing), (app, flow) =>
    {
        // The rule the arrangement exists for, and the one that fails silently: the list raises its
        // click on the first press, so the pane opens the row before anything knows a window was
        // wanted, and it is put back afterwards.
        app.RowClicks(OtherSubject);
        Assert.True(flow.PaneSubject() == OtherSubject, $"the pane starts on the other message, not '{flow.PaneSubject()}'");
        app.RowClicks(RowSubject, times: 2);
        app.WaitWindow(RowSubject);
        Assert.True(flow.PaneSubject() == OtherSubject,
            $"the pane keeps the message it had, not '{flow.PaneSubject()}': a window is a second reader, not a replacement");
    });

    [HarnessFact]
    public void AndTheWindowStaysInFrontRatherThanSinkingBehindTheMailbox() => Case(nameof(AndTheWindowStaysInFrontRatherThanSinkingBehindTheMailbox), (app, _) =>
    {
        // The symptom read from outside, twice: the first read lands before the snapshots the
        // double-click set off, and the window has to still be in front once they have arrived.
        app.RowClicks(OtherSubject);
        app.RowClicks(RowSubject, times: 2);
        app.WaitWindow(RowSubject);
        Assert.Equal(RowSubject, Desktop.ForegroundTitle());
        Thread.Sleep(TimeSpan.FromSeconds(2));
        Assert.True(Desktop.ForegroundTitle() == RowSubject,
            "and still is a further two seconds later, with the mailbox done reconciling behind it");
        // Asking the focus manager to search a window whose tree has not loaded once threw out of
        // the double-click handler and took the process with it.
        Assert.True(app.Running, "and the app is still running");
    });

    [HarnessFact]
    public void AnUnreadMessageOpensInAWindowThoughBeingMarkedReadRewritesItsRow() => Case(nameof(AnUnreadMessageOpensInAWindowThoughBeingMarkedReadRewritesItsRow), (app, flow) =>
    {
        // The case above passes on a message already read. Opening an unread one marks it read,
        // which rewrites its row under the gesture: the double-click once opened nothing, and a
        // window that did open lost the front.
        if (app.RowMenuItem(RowSubject, "action_mark_unread") is { } unread)
        {
            app.Agent.Invoke(unread);
        }
        else
        {
            Input.Press(Input.Escape);
        }
        // The mark-unread rewrite lands first, so the only rewrite left is the one the double-click
        // itself causes; otherwise the row moves under the pointer mid-gesture.
        Thread.Sleep(TimeSpan.FromSeconds(3));
        app.RowClicks(RowSubject, times: 2);
        app.WaitWindow(RowSubject);
        Assert.True(Desktop.ForegroundTitle() == RowSubject, "the window is in front when it opens");
        Thread.Sleep(TimeSpan.FromSeconds(4));
        Assert.True(Desktop.ForegroundTitle() == RowSubject, "and is still in front once the message has been marked read");
        Assert.True(app.Running, "and the app is still running");
    });

    [HarnessFact]
    public void ARegularMessageStaysInFrontAfterItsBodyHasBeenInThePane() => Case(nameof(ARegularMessageStaysInFrontAfterItsBodyHasBeenInThePane), (app, flow) =>
    {
        // The reported recipe: a regular message, whose body is a WebView2 document, rendered in the
        // pane and then double-clicked. The invitation the other cases use has a native card, and
        // that is exactly the message type the fault spares.
        app.RowClicks(OtherSubject);
        Assert.True(flow.PaneSubject() == OtherSubject, "the pane is showing the message first");
        Thread.Sleep(TimeSpan.FromSeconds(2));
        app.RowClicks(OtherSubject, times: 2);
        app.WaitWindow(OtherSubject);
        Assert.True(Desktop.ForegroundTitle() == OtherSubject, "the window is in front when it opens");
        Thread.Sleep(TimeSpan.FromSeconds(5));
        Assert.True(Desktop.ForegroundTitle() == OtherSubject, "and is still in front five seconds later");
        Assert.True(app.Running, "and the app is still running");
    });

    [HarnessFact]
    public void TheMailboxDoesNotClimbOverAWindowWhileItOpens() => Case(nameof(TheMailboxDoesNotClimbOverAWindowWhileItOpens), (app, _) =>
    {
        // Checked from the app's own log, because the steal it guards against lasted tens of
        // milliseconds and was over before a screen read landed. The mailbox takes no activation
        // after a window opens, and the shell putting the window back is not an excuse for one.
        // The pane on a different message, so the double-click's restore actually runs.
        app.RowClicks(RowSubject);
        var since = DateTimeOffset.Now;
        app.RowClicks(OtherSubject, times: 2);
        app.WaitWindow(OtherSubject);
        Thread.Sleep(TimeSpan.FromSeconds(4));
        DateTimeOffset? openedAt = null;
        int opened = 0, gaveTheFront = 0;
        var took = new List<string>();
        foreach (var line in AppLog.NewestSession().Split('\n'))
        {
            var stamp = Regex.Match(line, @"^(\S+ \S+ \S+) ");
            if (!stamp.Success || !DateTimeOffset.TryParse(stamp.Groups[1].Value, CultureInfo.InvariantCulture, DateTimeStyles.None, out var at) || at < since)
            {
                continue;
            }
            if (line.Contains("reading window: opened", StringComparison.Ordinal))
            {
                openedAt = at;
                opened++;
            }
            if (openedAt is null || at < openedAt)
            {
                continue;
            }
            // Bringing the mailbox forward before the clicks is the suite's doing; only what follows
            // the window opening is the app's.
            if (Regex.IsMatch(line, "window order: mailbox (code|pointer)"))
            {
                took.Add(line.Trim());
            }
            if (line.Contains("window order: mailbox lost the front", StringComparison.Ordinal))
            {
                gaveTheFront++;
            }
        }
        Assert.True(opened > 0, "the app logged the window it opened");
        // A quiet log proves nothing on its own: a window that never took the front reports nothing.
        Assert.True(gaveTheFront > 0, "and the window took the front off it, so there was a front to steal");
        Assert.True(took.Count == 0, $"the mailbox took the front back after the window opened: {string.Join(" / ", took)}");
        Assert.True(Desktop.ForegroundTitle() == OtherSubject, "and the window is the one in front at the end");
    });

    [HarnessFact]
    public void TheWindowCarriesTheWholeActionRowOverflowLast() => Case(nameof(TheWindowCarriesTheWholeActionRowOverflowLast), (app, _) =>
    {
        app.RowClicks(RowSubject, times: 2);
        var window = app.WaitWindow(RowSubject);
        var lefts = new List<double>();
        foreach (var id in RowButtons)
        {
            var button = app.Agent.Find(Filter.ById(id) with { Window = window.Hwnd });
            Assert.True(button is not null, $"the window's action row carries #{id}");
            lefts.Add(button!.X);
        }
        Assert.True(lefts.SequenceEqual(lefts.Order()), "the row is drawn in the contract order, left to right");
    });

    [HarnessFact]
    public void TheWindowDrawsTheAppsOwnCaptionNamedAfterTheMessage() => Case(nameof(TheWindowDrawsTheAppsOwnCaptionNamedAfterTheMessage), (app, _) =>
    {
        // Until it drew its own caption, a message window did not look like this app: the system's
        // strip follows neither the appearance setting nor the brand.
        app.RowClicks(RowSubject, times: 2);
        var window = app.WaitWindow(RowSubject);
        var caption = app.Agent.Find(Filter.ById("AppTitleBar") with { Window = window.Hwnd });
        Assert.True(caption is not null, "the window's caption must be the app's own TitleBar control, the same one the mailbox carries");
        Assert.True(caption!.Name == RowSubject, "and it names the message, which is what tells two open windows apart in the OS's window list");
        // The three buttons the app does not draw still have to be as tall as the caption it does.
        var expected = Desktop.CaptionHeightDip() * Input.Scale(window.Hwnd);
        foreach (var (id, bounds) in Desktop.CaptionButtons(window.Hwnd))
        {
            Assert.True(Math.Abs(bounds.Height - expected) <= 1,
                $"the {id} button is {bounds.Height}px tall against a caption of {expected}px, so this window's caption was taken over only halfway");
        }
    });

    [HarnessFact]
    public void DoubleClickingTheSameMessageAgainRaisesItsWindowInsteadOfOpeningASecond() => Case(nameof(DoubleClickingTheSameMessageAgainRaisesItsWindowInsteadOfOpeningASecond), (app, _) =>
    {
        // One window per message: a second would hold a second copy of one body.
        app.RowClicks(RowSubject, times: 2);
        app.WaitWindow(RowSubject);
        app.RowClicks(RowSubject, times: 2);
        Assert.True(app.ExtraWindows().Count == 1, "still one window for this message");
    });

    [HarnessFact]
    public void TheContextMenuOpensOneTooWhichIsTheRouteWithoutAPointer() => Case(nameof(TheContextMenuOpensOneTooWhichIsTheRouteWithoutAPointer), (app, flow) =>
    {
        var item = app.RowMenuItem(RowSubject, "action_open_in_window");
        Assert.True(item is not null, "the row menu offers \"open in new window\"");
        app.Agent.Invoke(item!);
        app.WaitWindow(RowSubject);
        Assert.True(app.ExtraWindows().Count == 1, "the menu item opened the window");
    });

    [HarnessFact]
    public void ReplyingFromTheWindowOpensTheDraftInAWindowOfItsOwn() => Case(nameof(ReplyingFromTheWindowOpensTheDraftInAWindowOfItsOwn), (app, _) =>
    {
        // Never the shell's inline composer: a draft answering the window in front of you does not
        // belong in a different window behind it.
        app.RowClicks(RowSubject, times: 2);
        var window = app.WaitWindow(RowSubject);
        app.Agent.Invoke(app.Agent.Find(Filter.ById("ReadingReply") with { Window = window.Hwnd })
            ?? throw new InvalidOperationException("the window offers no Reply"));
        // The draft's window is titled by the core's derived Re: subject, so it is found as the extra
        // window that is not the reading one, which is also the assertion: the reading window stays.
        App.Eventually(() => app.ExtraWindows().Count >= 2, App.Patience);
        Assert.True(app.ExtraWindows().Count == 2, "the reading window is still open behind its reply");
        var draft = app.ExtraWindows().FirstOrDefault(w => w.Title != RowSubject);
        Assert.True(draft is not null, "the draft opened in a window of its own");
        Assert.True(app.Agent.Wait(Filter.ById("ToField") with { Window = draft!.Hwnd }, App.Patience) is not null,
            "and it is the full composer, not a stub");
        Assert.True(App.Eventually(() => Desktop.ForegroundTitle() == draft.Title, TimeSpan.FromSeconds(5)),
            $"the draft is the window in front, not '{Desktop.ForegroundTitle()}': it is what the user just asked for");
        // Every window this app opens wears the brand, the third one included.
        foreach (var extra in app.ExtraWindows())
        {
            Assert.True(Desktop.HasIcon(extra.Hwnd), $"the '{extra.Title}' window carries the app icon");
        }
    });

    [HarnessFact]
    public void AConversationHeaderOpensNoWindow() => Case(nameof(AConversationHeaderOpensNoWindow), (app, _) =>
    {
        // A window is for a message. A conversation header is not one, and keeps what a second
        // click already did there: collapsing the thread the first click expanded.
        var thread = app.RowTitled(ThreadSubject);
        var texts = app.Agent.FindAll(new Filter { Type = "Text", Under = thread });
        // Asserted to be a conversation rather than assumed: if the seed stops threading these two,
        // this fails here instead of passing over a flat row.
        Assert.True(texts.Any(t => Regex.IsMatch(t.Name, @"^\d+$")),
            $"'{ThreadSubject}' is a conversation row, carrying the count that makes it one");
        // On its subject, because the header expands on the first click and its sub-rows are drawn
        // below it: the row's centre would land on a sub-row, which is a message.
        var subject = app.Agent.Find(new Filter { Type = "Text", Under = thread, NameLike = ThreadSubject });
        Assert.True(subject is not null, $"the conversation row draws its subject, '{ThreadSubject}'");
        app.ShowMailbox();
        Input.Click(subject!, times: 2);
        app.Agent.Settle(floor: TimeSpan.FromSeconds(1));
        Assert.True(app.ExtraWindows().Count == 0, "a conversation is not a message, so double-clicking its header opens no window");
    });

    [HarnessFact]
    public void ClosingTheWindowLeavesThePaneWhereItWas() => Case(nameof(ClosingTheWindowLeavesThePaneWhereItWas), (app, flow) =>
    {
        app.RowClicks(OtherSubject);
        app.RowClicks(RowSubject, times: 2);
        app.WaitWindow(RowSubject);
        app.CloseExtraWindows();
        Assert.True(app.ExtraWindows().Count == 0, "the window closed");
        Assert.True(flow.PaneSubject() == OtherSubject, $"and the pane still holds its own message, not '{flow.PaneSubject()}'");
    });
}
