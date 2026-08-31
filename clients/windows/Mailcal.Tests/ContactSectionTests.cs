// The contacts list's pure decisions. Each of these is a rule docs/contacts.md binds every client
// to, and each of them fails *plausibly* rather than loudly if it is got wrong:
//
//   * re-bucket the core's ordered rows and the list is silently ordered twice, by two rules;
//   * recount the merge disclosure and every ordinary contact reads "In 1 accounts";
//   * show a raw account id and the detail leaks how ids are built;
//   * pick the wrong empty state and someone who just searched is told their contacts are gone.

using System.Collections.Generic;
using Allodia.Mailcal.Services;
using uniffi.mailcal_bindings;
using Xunit;

namespace Allodia.Mailcal.Tests;

public class ContactSectionTests
{
    private static ContactRow Row(string name, string section, uint accounts = 1) =>
        new(Id: name, DisplayName: name, PrimaryEmail: name + "@example.test",
            Section: section, Avatar: AvatarFixture.Core(name[..1]),
            AccountCount: accounts);

    private static ContactDetail Detail(params string[] accounts) =>
        new(Id: "p1", DisplayName: "Ada", Avatar: AvatarFixture.Core("A"),
            Emails: [], Phones: [], Organizations: [], Titles: [], Accounts: accounts);

    [Fact]
    public void TheFirstRowAlwaysCarriesItsSectionHeader()
    {
        List<ContactRow> rows = [Row("Ada", "A")];
        Assert.Equal("A", ContactSections.HeaderFor(rows, 0));
    }

    [Fact]
    public void OnlyTheFirstRowOfASectionCarriesTheHeader()
    {
        List<ContactRow> rows = [Row("Ada", "A"), Row("Alan", "A"), Row("Bob", "B")];
        Assert.Equal("A", ContactSections.HeaderFor(rows, 0));
        Assert.Null(ContactSections.HeaderFor(rows, 1));
        Assert.Equal("B", ContactSections.HeaderFor(rows, 2));
    }

    // Order in, order out: the header is decided by comparing with the row ABOVE, never by
    // re-bucketing on the section key. Re-grouping would be a second ordering that could disagree
    // with the core's, so a section that reappeared would get a second header rather than being
    // silently merged into the first, which is the honest rendering of a list we did not re-sort.
    [Fact]
    public void ARepeatedSectionGetsASecondHeaderRatherThanBeingRegrouped()
    {
        List<ContactRow> rows = [Row("Ada", "A"), Row("Bob", "B"), Row("Alan", "A")];
        Assert.Equal("A", ContactSections.HeaderFor(rows, 0));
        Assert.Equal("B", ContactSections.HeaderFor(rows, 1));
        Assert.Equal("A", ContactSections.HeaderFor(rows, 2));
    }

    [Fact]
    public void AnIndexOutsideTheListHasNoHeader()
    {
        List<ContactRow> rows = [Row("Ada", "A")];
        Assert.Null(ContactSections.HeaderFor(rows, 1));
        Assert.Null(ContactSections.HeaderFor(rows, -1));
        Assert.Null(ContactSections.HeaderFor([], 0));
    }

    // "In 1 accounts" on every ordinary contact is noise, and ungrammatical noise.
    [Fact]
    public void OnlyAMergedRowDisclosesItsAccounts()
    {
        Assert.False(ContactSections.DisclosesAccounts(0));
        Assert.False(ContactSections.DisclosesAccounts(1));
        Assert.True(ContactSections.DisclosesAccounts(2));
    }

    // The same asymmetry in the detail pane: with one account there is nothing to disambiguate, so
    // the per-value provenance tags are suppressed rather than repeating one name down the screen.
    [Fact]
    public void PerValueProvenanceShowsOnlyForAPersonSpanningSeveralAccounts()
    {
        Assert.False(ContactSections.SpansSeveralAccounts(Detail("work")));
        Assert.True(ContactSections.SpansSeveralAccounts(Detail("work", "home")));
    }

    // The core's ids are internal (`alice@test.local@jmap:127.0.0.1:18080`); the user knows their
    // accounts by address.
    [Fact]
    public void AnAccountIsNamedByTheAddressTheUserKnowsItBy()
    {
        var labels = new Dictionary<string, string> { ["a0"] = "alice@test.local" };
        Assert.Equal("alice@test.local", ContactSections.AccountLabel("a0", labels));
    }

    // An account removed since the snapshot falls back to its id rather than vanishing: a value with
    // no visible source is worse than an ugly one.
    [Fact]
    public void AnAccountWithNoLabelLeftFallsBackToItsId() =>
        Assert.Equal("a9", ContactSections.AccountLabel("a9", new Dictionary<string, string>()));

    [Fact]
    public void SeveralAccountsAreJoinedForOneProvenanceCaption()
    {
        var labels = new Dictionary<string, string>
        {
            ["a0"] = "alice@test.local",
            ["a1"] = "bob@test.local",
        };
        Assert.Equal(
            "alice@test.local, bob@test.local",
            ContactSections.AccountLabels(["a0", "a1"], labels));
    }

    [Fact]
    public void PeopleOnScreenMeanTheListShows() =>
        Assert.Equal(ContactsListState.Rows, ContactSections.ListState(3, string.Empty));

    // A narrowed list is still the list, however few people it holds.
    [Fact]
    public void PeopleOnScreenUnderASearchStillMeanTheListShows() =>
        Assert.Equal(ContactsListState.Rows, ContactSections.ListState(1, "ada"));

    // The two empty states are deliberately different sentences: "No contacts yet" shown to someone
    // who has just searched reads as though theirs had vanished.
    [Fact]
    public void AnEmptyListWithNoSearchMeansNothingHasSyncedYet() =>
        Assert.Equal(ContactsListState.NoContacts, ContactSections.ListState(0, string.Empty));

    [Fact]
    public void AnEmptyListUnderASearchMeansNoMatches() =>
        Assert.Equal(ContactsListState.NoResults, ContactSections.ListState(0, "zzz"));

    // A query of nothing but spaces is not a search, it narrows nothing in the core, so claiming
    // "no contacts match your search" would name a search the user did not make.
    [Fact]
    public void AWhitespaceQueryIsNotASearch() =>
        Assert.Equal(ContactsListState.NoContacts, ContactSections.ListState(0, "   "));
}
