// "The organiser wasn't told", the question the core raises when the calendar server stored the RSVP
// and reported it could not pass it on (RFC 6638 §3.2.9; docs/invitations.md, "A server can promise
// to send the reply, and then not send it"), drawn as a notice in the window's bar with its two
// answers (docs/notices.md rule 8).
//
// WHY A FAKE VERDICT. Stalwart delivers replies and reports nothing, and the server known to report
// 5.2 is somebody's production account. MAILCAL_FAKE_REPLY_DELIVERY substitutes the verdict and
// nothing else: the core raises the question, signals the notices, follows the account's standing
// answer and withdraws the question when it is answered. On the CalDAV half, because the verdict is
// read only on the server delivery route.
//
// WHY IT DECLINES. The RSVP is real and lands on the shared server. "Quarterly planning" must stay
// unanswered for AttendeesTests, and "Weekend walk" is the free day InvitationPreviewTests needs
// empty: a declined event is hidden from every calendar surface, so declining it leaves that day as
// empty as it was, where accepting puts an event on it.
//
// Every string is matched against the catalog in any locale. The cases build on one another and run
// in the order written.

using Allodia.Mailcal.UiTests.Driver;
using Xunit;

namespace Allodia.Mailcal.UiTests.Suites;

[Collection(FakeReplyHarnessCollection.Name)]
[Trait("Dataset", "harness")]
public sealed class InvitationReplyPromptTests(FakeReplyHarnessApp harness)
{
    private const string Invitation = "Weekend walk";
    private const string Organiser = "bob@test.local";

    /// <summary>The bar while it puts this question, or null.</summary>
    private static Element? Bar(App app) =>
        app.NoticeBar() is { } bar && ReplyPromptFlow.ShowsBody(ReplyPromptFlow.Texts(app, bar), Invitation, Organiser) ? bar : null;

    /// <summary>The bar putting the question, waited for: the RSVP is a real round trip to the
    /// harness.</summary>
    private static Element WaitBar(App app)
    {
        Element? bar = null;
        return App.Eventually(() => (bar = Bar(app)) is not null, TimeSpan.FromSeconds(60))
            ? bar!
            : throw new InvalidOperationException(
                "no notice says the reply was not passed on within 60 s. In this order: the answer never went out (a stale store leaves the invitation with no card, scripts/dev/harness.sh reset); this is not a Debug build, so the fake verdict is compiled out; a standing answer on the account answered for the user; the bar does not follow Surface.Notices");
    }

    /// <summary>Opens the invitation and declines it.</summary>
    private static void Decline(App app)
    {
        app.OpenMessage(Invitation);
        var labels = Catalog.Everywhere("a11y_invitation_decline");
        Element? decline = null;
        if (!App.Eventually(() => (decline = app.Agent.FindAll(new Filter { Type = "Button", Window = app.Mailbox.Hwnd, OnScreen = true })
                .FirstOrDefault(b => labels.Contains(b.Name))) is not null, TimeSpan.FromSeconds(45)))
        {
            throw new InvalidOperationException(
                $"'{Invitation}' drew no invitation card, so there was nothing to answer; a stale client store is the usual cause (scripts/dev/harness.sh reset)");
        }
        app.Agent.Invoke(decline!);
    }

    [HarnessFact]
    public void AReportedFailureRaisesTheNoticeNamingTheMeetingAndThePersonWhoWouldBeEmailed() =>
        harness.Case(nameof(AReportedFailureRaisesTheNoticeNamingTheMeetingAndThePersonWhoWouldBeEmailed), app =>
        {
            ReplyPromptFlow.ForgetRemembered();
            Decline(app);
            // WaitBar matches the whole sentence: the organiser's address rather than "the
            // organiser", since a person consenting to send mail as themselves is entitled to see
            // the recipient.
            WaitBar(app);
        });

    [HarnessFact]
    public void TheRfcStatusCodeIsNowhereOnScreen() => harness.Case(nameof(TheRfcStatusCodeIsNowhereOnScreen), app =>
    {
        // This suite chose the token, so finding it means the code left the log for a notice, where
        // it explains nothing to the person reading it.
        var shown = app.Agent.FindAll(new Filter { Under = WaitBar(app) })
            .Select(e => e.Name).Where(n => n.Contains(FakeReplyHarnessApp.Status, StringComparison.Ordinal)).ToList();
        Assert.True(shown.Count == 0, $"the status code rides the question for the log, never for the user; found it in: {string.Join(" | ", shown)}");
    });

    [HarnessFact]
    public void TheNoticeOffersItsTwoAnswersAndCannotBeClosed() => harness.Case(nameof(TheNoticeOffersItsTwoAnswersAndCannotBeClosed), app =>
    {
        // Nothing may close the question unanswered, or the core holds one nobody can see.
        var bar = WaitBar(app);
        Assert.Contains(app.NoticeAnswer(bar)?.Name ?? "", Catalog.Everywhere("notice_action_send_reply"));
        Assert.Contains(app.NoticeSecondAnswer(bar)?.Name ?? "", Catalog.Everywhere("notice_action_dont_send_reply"));
        Assert.True(app.NoticeClose(bar) is null, "a question is answered, never closed");
    });

    [HarnessFact]
    public void AnsweringTakesItAwayAndAStandingNoOnTheAccountPageStopsTheNextMeetingAsking() => harness.Case(nameof(AnsweringTakesItAwayAndAStandingNoOnTheAccountPageStopsTheNextMeetingAsking), app =>
    {
        app.Agent.Invoke(app.NoticeSecondAnswer(WaitBar(app)) ?? throw new InvalidOperationException("the question offers no Don't send"));
        Assert.True(App.Eventually(() => Bar(app) is null, TimeSpan.FromSeconds(30)),
            "the core withdraws the question as it is answered; a notice still standing means the bar does not follow it");

        // A standing no, set where it now lives, and dropped, shows only as being asked forever on
        // the server the setting exists for.
        var dialog = new SetupFlow(app).OpenAccount(Harness.Alice);
        var picker = app.Agent.Wait(new Filter { Id = "AccountReplyFallback", Type = "ComboBox", Under = dialog }, TimeSpan.FromSeconds(10))
            ?? throw new InvalidOperationException("the account's page offers no reply setting (#AccountReplyFallback)");
        app.Agent.Expand(picker);
        // The combo also lists its items under itself; the open list's own are the ones drawn, and
        // neither takes a selection through its peer, so the choice is a click, as a person makes it.
        var never = Catalog.Everywhere("settings_reply_fallback_never");
        Element? item = null;
        Assert.True(App.Eventually(() =>
        {
            var closed = app.Agent.FindAll(new Filter { Type = "ListItem", Under = picker }).Select(e => e.Handle).ToHashSet();
            return (item = app.Agent.FindAll(new Filter { Type = "ListItem" })
                .FirstOrDefault(i => !closed.Contains(i.Handle) && never.Contains(i.Name))) is not null;
        }, TimeSpan.FromSeconds(5)), "the reply setting offers Don't email them");
        Input.Click(app.Steady(item!));
        Assert.True(App.Eventually(ReplyPromptFlow.Remembered, TimeSpan.FromSeconds(10)), "the choice is stored as the account's standing answer");
        app.CloseDialogs();

        Decline(app);
        app.Agent.Settle(quiet: TimeSpan.FromSeconds(1), floor: TimeSpan.FromSeconds(6), cap: TimeSpan.FromSeconds(10));
        Assert.True(Bar(app) is null, "a standing no stops the next meeting asking");
    }, app =>
    {
        app.CloseDialogs();
        ReplyPromptFlow.ForgetRemembered();
    });
}
