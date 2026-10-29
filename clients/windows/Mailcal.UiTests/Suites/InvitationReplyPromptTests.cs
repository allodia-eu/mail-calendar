// "The organiser wasn't told", the bar the window raises when the calendar server stored the RSVP and
// reported it could not pass it on (RFC 6638 §3.2.9; docs/invitations.md, "A server can promise to
// send the reply, and then not send it").
//
// WHY A FAKE VERDICT. Stalwart delivers replies and reports nothing, and the server known to report
// 5.2 is somebody's production account. MAILCAL_FAKE_REPLY_DELIVERY substitutes the verdict and
// nothing else: the core raises the question, signals the surface, remembers what it is told and
// clears itself when answered. On the CalDAV half, because the verdict is read only on the server
// delivery route.
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
[TestCaseOrderer(DeclarationOrderer.Name, DeclarationOrderer.Assembly)]
[Trait("Dataset", "harness")]
public sealed class InvitationReplyPromptTests(FakeReplyHarnessApp harness)
{
    private const string Invitation = "Weekend walk";
    private const string Organiser = "bob@test.local";

    private static readonly Filter BarFilter = Filter.ById("InvitationReplyPrompt") with { OnScreen = true };

    private static Element? Bar(App app) => app.Agent.Find(BarFilter);

    /// <summary>The bar, waited for: the RSVP is a real round trip to the harness.</summary>
    private static Element WaitBar(App app) =>
        app.Agent.Wait(BarFilter, TimeSpan.FromSeconds(60))
        ?? throw new InvalidOperationException(
            "no reply-undelivered bar within 60 s. In this order: the answer never went out (a stale store leaves the invitation with no card, scripts/dev/harness.sh reset); this is not a Debug build, so the fake verdict is compiled out; the surface no longer reaches the projection; the InfoBar's IsOpen is not wired");

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

    private static Element Tick(App app, Element bar) =>
        app.Agent.Find(new Filter { Type = "CheckBox", Under = bar }) ?? throw new InvalidOperationException("the bar offers no remembered-choice tick");

    private static bool WaitClosed(App app) => app.Agent.WaitGone(BarFilter, TimeSpan.FromSeconds(30));

    [HarnessFact]
    public void AReportedFailureRaisesTheBarNamingTheMeetingAndThePersonWhoWouldBeEmailed() =>
        harness.Case(nameof(AReportedFailureRaisesTheBarNamingTheMeetingAndThePersonWhoWouldBeEmailed), app =>
        {
            ReplyPromptFlow.ForgetRemembered();
            Decline(app);
            var bar = WaitBar(app);
            // The organiser's address rather than "the organiser": a person consenting to send mail
            // as themselves is entitled to see the recipient.
            Assert.True(ReplyPromptFlow.ShowsBody(ReplyPromptFlow.Texts(app, bar), Invitation, Organiser),
                $"the bar shows invitation_reply_undelivered_body with the meeting and the organiser in its slots; it shows: {string.Join(" | ", ReplyPromptFlow.Texts(app, bar))}");
        });

    [HarnessFact]
    public void TheRfcStatusCodeIsNowhereOnScreen() => harness.Case(nameof(TheRfcStatusCodeIsNowhereOnScreen), app =>
    {
        // This suite chose the token, so finding it means the code left the log for a prompt, where
        // it explains nothing to the person reading it.
        var shown = app.Agent.FindAll(new Filter { Under = WaitBar(app) })
            .Select(e => e.Name).Where(n => n.Contains(FakeReplyHarnessApp.Status, StringComparison.Ordinal)).ToList();
        Assert.True(shown.Count == 0, $"the status code rides the prompt for the log, never for the user; found it in: {string.Join(" | ", shown)}");
    });

    [HarnessFact]
    public void TheRememberTickIsPresentLabelledFromTheCatalogAndStartsClear() => harness.Case(nameof(TheRememberTickIsPresentLabelledFromTheCatalogAndStartsClear), app =>
    {
        // Never ticked in advance: beside "Send the email" a tick is standing permission to send
        // mail as the user.
        var tick = Tick(app, WaitBar(app));
        Assert.Contains(tick.Name, Catalog.Everywhere("invitation_reply_undelivered_remember"));
        Assert.Equal("Off", app.Agent.Read(tick).Toggle);
    });

    [HarnessFact]
    public void TheBarOffersExactlyTwoWaysOutAndNeitherClosesItUnanswered() => harness.Case(nameof(TheBarOffersExactlyTwoWaysOutAndNeitherClosesItUnanswered), app =>
    {
        // Nothing may dismiss the question unanswered, or the core holds a prompt nobody can see:
        // IsClosable="False" enforces it, and nothing else watches that property.
        var bar = WaitBar(app);
        var names = app.Agent.FindAll(new Filter { Type = "Button", Under = bar, OnScreen = true }).Select(b => b.Name).ToList();
        Assert.True(names.Count == 2, $"the bar offers the two answers and no third way out; found: {string.Join(" | ", names)}");
        Assert.True(ReplyPromptFlow.Button(app, bar, "invitation_reply_undelivered_send") is not null, $"one answer sends the email; found: {string.Join(" | ", names)}");
        Assert.True(ReplyPromptFlow.Button(app, bar, "invitation_reply_undelivered_dismiss") is not null, $"one answer does not; found: {string.Join(" | ", names)}");
    });

    [HarnessFact]
    public void AnsweringClosesItAndARememberedNoStopsTheNextMeetingAsking() => harness.Case(nameof(AnsweringClosesItAndARememberedNoStopsTheNextMeetingAsking), app =>
    {
        // A standing no, dropped, shows only as being asked forever on the server the setting
        // exists for; the second half needs the first to have happened in this process.
        var bar = WaitBar(app);
        app.Agent.Toggle(Tick(app, bar));
        app.Agent.Invoke(ReplyPromptFlow.Button(app, bar, "invitation_reply_undelivered_dismiss")!);
        Assert.True(WaitClosed(app), "the core clears the question as it is answered; a bar still standing means the client does not follow it");
        Assert.True(App.Eventually(ReplyPromptFlow.Remembered, TimeSpan.FromSeconds(5)), "the tick beside \"Don't send\" is stored as the account's standing answer");
        Decline(app);
        app.Agent.Settle(quiet: TimeSpan.FromSeconds(1), floor: TimeSpan.FromSeconds(6), cap: TimeSpan.FromSeconds(10));
        Assert.True(Bar(app) is null, "a remembered no stops the next meeting asking");
    });

    [HarnessFact]
    public void TheTickDoesNotCarryOverToTheNextMeeting() => harness.Case(nameof(TheTickDoesNotCarryOverToTheNextMeeting), app =>
    {
        // An InfoBar's content is not created again when it closes and opens, so a tick left from
        // the meeting before would set a standing choice the user was never shown. It has to run in
        // the process that ticked it, after the case above.
        ReplyPromptFlow.ForgetRemembered();
        Decline(app);
        var bar = WaitBar(app);
        Assert.Equal("Off", app.Agent.Read(Tick(app, bar)).Toggle);
        app.Agent.Invoke(ReplyPromptFlow.Button(app, bar, "invitation_reply_undelivered_dismiss")!);
        Assert.True(WaitClosed(app), "and it still closes");
    }, _ => ReplyPromptFlow.ForgetRemembered());
}
