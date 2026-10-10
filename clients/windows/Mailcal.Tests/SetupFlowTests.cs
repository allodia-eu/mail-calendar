// The last setup step (docs/onboarding.md, "After the connect"): which accounts a new one is
// offered to link to, that a suggestion starts picked and survives only until the person changes
// a pick, and that "Add another account" comes back to the step it left.

using Allodia.Mailcal.Services;
using uniffi.mailcal_bindings;
using Xunit;

namespace Allodia.Mailcal.Tests;

public class SetupFlowTests
{
    private static AccountsSnapshot Snapshot(AccountKind kind = AccountKind.Imap, bool suggested = true) => new([
        new AccountEntry(
            Id: "alice@imap",
            Address: "alice@example.org",
            Kind: kind,
            Uses: [new AccountUse(AccountCapability.Mail, CapabilityState.On)],
            Links: new AccountLinksView(null, null, null),
            LinkedFrom: [],
            LinkCandidates: new LinkCandidates(
                [AccountSettingsRulesTests.Linked("bob@dav"), AccountSettingsRulesTests.Linked("alice@dav")],
                [], [], suggested ? ["alice@dav"] : []),
            Endpoints: null),
    ]);

    private static bool Standards(AccountKind kind) => kind is AccountKind.Imap or AccountKind.Dav;

    private static Func<string, SetupLinkStep?> StepsOf(AccountsSnapshot snapshot) =>
        account => SetupLinkStep.For(snapshot, account, Standards);

    [Fact]
    public void ASuggestionStartsPickedAndContinuingLinksIt()
    {
        var step = SetupLinkStep.For(Snapshot(), "alice@imap", Standards);
        Assert.NotNull(step);
        Assert.Single(step.Pickers);
        Assert.Equal(new int?[] { 1 }, step.Picked);
        Assert.Equal([(LinkSlot.Calendar, "alice@dav")], step.Links());
    }

    [Fact]
    public void NothingToLinkIsNoStep()
    {
        Assert.Null(SetupLinkStep.For(Snapshot(), "someone-else", Standards));
        // A provider account is linked from Settings, not offered it at setup.
        Assert.Null(SetupLinkStep.For(Snapshot(AccountKind.Google), "alice@imap", Standards));
        var lone = new AccountsSnapshot([Snapshot().Accounts[0] with { LinkCandidates = new LinkCandidates([], [], [], []) }]);
        Assert.Null(SetupLinkStep.For(lone, "alice@imap", Standards));
    }

    [Fact]
    public void APickSurvivesALateSuggestionAndNoneLinksNothing()
    {
        var step = SetupLinkStep.For(Snapshot(suggested: false), "alice@imap", Standards)!;
        Assert.Equal(new int?[] { null }, step.Picked);
        Assert.Empty(step.Links());

        // Untouched, the step takes the suggestion that arrived.
        step = step.Refreshed(SetupLinkStep.For(Snapshot(), "alice@imap", Standards)!);
        Assert.Equal(new int?[] { 1 }, step.Picked);

        // Touched, it keeps what the person picked.
        step.Pick(0, 0);
        var kept = step.Refreshed(SetupLinkStep.For(Snapshot(), "alice@imap", Standards)!);
        Assert.Equal([(LinkSlot.Calendar, "bob@dav")], kept.Links());
    }

    [Fact]
    public void AnotherAccountComesBackToTheStepItWasAddedFrom()
    {
        var steps = StepsOf(Snapshot());
        var flow = new SetupFlow();
        Assert.True(flow.AccountAdded("alice@imap", steps));
        Assert.Equal("alice@imap", flow.Links?.Account);

        flow.AddLinked();
        Assert.Null(flow.Links);
        Assert.Equal("alice@imap", flow.ReturningTo);

        // The account added from there brings the person back to the step they left, not to one
        // of its own.
        Assert.True(flow.AccountAdded("alice@dav", steps));
        Assert.Equal("alice@imap", flow.Links?.Account);
        Assert.Equal(["alice@imap", "alice@dav"], flow.Finish());
        Assert.Null(flow.Links);
        Assert.Empty(flow.Added);
    }

    [Fact]
    public void CancellingAnotherAccountReturnsToTheStepAndAFlowWithNothingToLinkEnds()
    {
        var flow = new SetupFlow();
        flow.AccountAdded("alice@imap", StepsOf(Snapshot()));
        flow.AddLinked();
        Assert.True(flow.Cancelled(StepsOf(Snapshot())));
        Assert.Equal("alice@imap", flow.Links?.Account);

        // Nothing left to return to: the caller ends the flow, and still asks each added name.
        Assert.False(flow.Cancelled(StepsOf(Snapshot())));
        Assert.Equal(["alice@imap"], flow.Finish());

        var lonely = new SetupFlow();
        Assert.False(lonely.AccountAdded("someone-else", StepsOf(Snapshot())));
    }

    [Fact]
    public void AnUnchangedStepIsNotRedrawnAndALateSuggestionIs()
    {
        var unsuggested = StepsOf(Snapshot(suggested: false));
        var flow = new SetupFlow();
        flow.AccountAdded("alice@imap", unsuggested);
        Assert.False(flow.Refresh(unsuggested("alice@imap")));
        Assert.True(flow.Refresh(StepsOf(Snapshot())("alice@imap")));
        Assert.Equal(new int?[] { 1 }, flow.Links!.Picked);
    }
}
