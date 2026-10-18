// The line the calendar and contacts screens draw over a use that did not connect (docs/accounts.md
// rule 16, docs/calendar.md §4, docs/contacts.md §3).

using Allodia.Mailcal.Services;
using uniffi.mailcal_bindings;
using Xunit;

namespace Allodia.Mailcal.Tests;

public class UseOutageLinesTests
{
    // Words that say which key they stand for, so a test reads which sentence went where.
    private static readonly UseOutageWords Words = new(
        address => $"calendar_outage_title({address})",
        address => $"contacts_outage_title({address})",
        note => $"reason:{note}",
        "use_outage_last_synced");

    private static string? AddressOf(string id) => id == "alice-id" ? "alice@example.org" : null;

    [Theory]
    [InlineData(nameof(UseProblem.NeedsOwnSignIn), nameof(AccountCapability.Calendar), "calendar_outage_title(alice@example.org) reason:OwnSignInCalendar use_outage_last_synced")]
    [InlineData(nameof(UseProblem.SignInRefused), nameof(AccountCapability.Calendar), "calendar_outage_title(alice@example.org) reason:RefusedCalendar use_outage_last_synced")]
    [InlineData(nameof(UseProblem.Unreachable), nameof(AccountCapability.Calendar), "calendar_outage_title(alice@example.org) reason:UnreachableCalendar use_outage_last_synced")]
    [InlineData(nameof(UseProblem.NeedsOwnSignIn), nameof(AccountCapability.Contacts), "contacts_outage_title(alice@example.org) reason:OwnSignInContacts use_outage_last_synced")]
    [InlineData(nameof(UseProblem.SignInRefused), nameof(AccountCapability.Contacts), "contacts_outage_title(alice@example.org) reason:RefusedContacts use_outage_last_synced")]
    [InlineData(nameof(UseProblem.Unreachable), nameof(AccountCapability.Contacts), "contacts_outage_title(alice@example.org) reason:UnreachableContacts use_outage_last_synced")]
    public void TheLineIsTheTitleThenTheAccountPagesReasonThenLastSynced(
        string problem, string use, string expected)
    {
        // Named rather than typed: the generated enums are internal, and a public test cannot take one.
        var lines = UseOutageLines.Of(
            [new UseOutage("alice-id", Enum.Parse<UseProblem>(problem))],
            Enum.Parse<AccountCapability>(use),
            AddressOf,
            Words);

        var line = Assert.Single(lines);
        Assert.Equal("alice-id", line.AccountId);
        Assert.Equal(expected, line.Text);
    }

    [Fact]
    public void TheReasonIsTheOneTheAccountsPageGives()
    {
        foreach (var problem in Enum.GetValues<UseProblem>())
        {
            foreach (var use in new[] { AccountCapability.Calendar, AccountCapability.Contacts })
            {
                var text = UseOutageLines.Text(new UseOutage("alice-id", problem), use, "alice@example.org", Words);
                Assert.Contains($"reason:{AccountSettingsRules.ProblemNote(use, problem)}", text);
            }
        }
    }

    [Fact]
    public void OneLinePerAccountInTheSnapshotsOrder()
    {
        UseOutage[] outages =
        [
            new("bob-id", UseProblem.Unreachable),
            new("alice-id", UseProblem.SignInRefused),
        ];

        var lines = UseOutageLines.Of(outages, AccountCapability.Calendar, AddressOf, Words);

        Assert.Equal(["bob-id", "alice-id"], lines.Select(line => line.AccountId));
        // An account no snapshot entry carries is named by its id rather than not at all.
        Assert.StartsWith("calendar_outage_title(bob-id)", lines[0].Text);
        Assert.StartsWith("calendar_outage_title(alice@example.org)", lines[1].Text);
    }

    [Fact]
    public void ASnapshotWithoutOutagesDrawsNoLine()
    {
        Assert.Empty(UseOutageLines.Of([], AccountCapability.Calendar, AddressOf, Words));
        Assert.Empty(UseOutageLines.Of([], AccountCapability.Contacts, AddressOf, Words));
    }

    [Fact]
    public void ASnapshotWithOutagesDrawsTheirLinesAndOneWithoutClearsThem()
    {
        var snapshot = Connectivity(
            calendar: [new UseOutage("alice-id", UseProblem.NeedsOwnSignIn)],
            contacts: [new UseOutage("alice-id", UseProblem.SignInRefused)]);
        Assert.Single(UseOutageLines.Of(snapshot.CalendarOutages, AccountCapability.Calendar, AddressOf, Words));
        Assert.Single(UseOutageLines.Of(snapshot.ContactsOutages, AccountCapability.Contacts, AddressOf, Words));

        var cleared = Connectivity(calendar: [], contacts: []);
        Assert.Empty(UseOutageLines.Of(cleared.CalendarOutages, AccountCapability.Calendar, AddressOf, Words));
        Assert.Empty(UseOutageLines.Of(cleared.ContactsOutages, AccountCapability.Contacts, AddressOf, Words));
    }

    [Fact]
    public void AnUnchangedSignalLeavesTheLinesAlone()
    {
        var first = UseOutageLines.Of([new UseOutage("alice-id", UseProblem.Unreachable)], AccountCapability.Calendar, AddressOf, Words);
        var same = UseOutageLines.Of([new UseOutage("alice-id", UseProblem.Unreachable)], AccountCapability.Calendar, AddressOf, Words);
        var moved = UseOutageLines.Of([new UseOutage("alice-id", UseProblem.SignInRefused)], AccountCapability.Calendar, AddressOf, Words);

        Assert.True(UseOutageLines.Same(first, same));
        Assert.False(UseOutageLines.Same(first, moved));
        Assert.False(UseOutageLines.Same(first, []));
    }

    private static ConnectivitySnapshot Connectivity(UseOutage[] calendar, UseOutage[] contacts) =>
        new(false, [], [], [], [], calendar, contacts);
}
