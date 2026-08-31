// The composer's recipient-field string work, the half of autosuggest that has nothing to do with
// ranking and everything to do with the fact that To/Cc/Bcc hold a LIST in one string.
//
// Two of these tests exist because the failures are silent, and both are the ones docs/contacts.md
// §4 binds every client to: querying the whole field matches nothing once a first recipient is
// entered, and replacing the whole field on selection destroys every recipient already typed. A
// third pins the trimmed-comparison invariant the field's re-seed guard depends on, compare raw
// and every space the user types is eaten, so "John Smith" arrives as "JohnSmith" and a name query
// can never match.

using Allodia.Mailcal.Services;
using Xunit;

namespace Allodia.Mailcal.Tests;

public class RecipientTokenTests
{
    // The completion target is the text after the LAST comma, not the field. This is the silent
    // one: query the whole field and the core matches nothing the moment a first recipient exists.
    [Fact]
    public void TheTokenIsWhatFollowsTheLastComma()
    {
        Assert.Equal("gr", RecipientTokens.CurrentToken("ada@example.test, gr"));
        Assert.Equal("ada", RecipientTokens.CurrentToken("ada"));
        Assert.Equal(string.Empty, RecipientTokens.CurrentToken(string.Empty));
    }

    // A field that ends at a separator is between recipients: the token is empty, the core answers
    // nothing, and the list closes rather than offering everyone.
    [Fact]
    public void AFieldEndingAtASeparatorHasNoToken() =>
        Assert.Equal(string.Empty, RecipientTokens.CurrentToken("ada@example.test, "));

    [Fact]
    public void FinishedRecipientsAreEverythingBeforeTheLastComma()
    {
        Assert.Equal(
            new[] { "ada@example.test", "grace@example.test" },
            RecipientTokens.Committed("ada@example.test, grace@example.test, li"));
        // Nothing is finished until there is a comma, the first address is still being typed.
        Assert.Empty(RecipientTokens.Committed("ada@example.test"));
    }

    // A stray ",," or a trailing separator must not become a blank pill with a remove button that
    // takes out the wrong recipient.
    [Fact]
    public void EmptyEntriesNeverBecomePills() =>
        Assert.Equal(
            new[] { "ada@example.test" },
            RecipientTokens.Committed("ada@example.test,, , li"));

    // The other silent one: accepting a suggestion must replace ONLY the token. Replacing the field
    // destroys every recipient already entered, and the user finds out when the send goes to one
    // person.
    [Fact]
    public void AcceptingASuggestionKeepsTheEarlierRecipients() =>
        Assert.Equal(
            "ada@example.test, grace@example.test, ",
            RecipientTokens.Accept("ada@example.test, gr", "grace@example.test"));

    // The trailing separator is the point: the caret lands after it, so the next recipient can be
    // typed without reaching for the comma key, and the accepted address is now a finished pill.
    [Fact]
    public void AnAcceptedAddressBecomesAFinishedRecipient()
    {
        var field = RecipientTokens.Accept("gr", "grace@example.test");
        Assert.Equal(new[] { "grace@example.test" }, RecipientTokens.Committed(field));
        Assert.Equal(string.Empty, RecipientTokens.CurrentToken(field));
    }

    // Addresses go in BARE. A display name containing a comma would split into two invalid
    // recipients, which the user would not discover until the send failed.
    [Fact]
    public void OnlyTheAddressIsInserted() =>
        Assert.Equal("grace@example.test, ", RecipientTokens.Accept(string.Empty, "grace@example.test"));

    [Fact]
    public void RemovingAPillTakesTheRightOneAndKeepsTheTokenBeingTyped()
    {
        var field = "ada@example.test, grace@example.test, li";
        Assert.Equal("grace@example.test, li", RecipientTokens.Remove(field, 0));
        Assert.Equal("ada@example.test, li", RecipientTokens.Remove(field, 1));
    }

    // The pill list and a click on it are a frame apart; a re-render in between must not crash the
    // composer.
    [Theory]
    [InlineData(-1)]
    [InlineData(2)]
    public void AnOutOfRangeRemoveLeavesTheFieldAlone(int index) =>
        Assert.Equal(
            "ada@example.test, li",
            RecipientTokens.Remove("ada@example.test, li", index));

    [Fact]
    public void TheFieldTextIsRebuiltFromThePillsPlusTheToken()
    {
        Assert.Equal(
            "ada@example.test, grace@example.test, li",
            RecipientTokens.FieldText(["ada@example.test", "grace@example.test"], "li"));
        Assert.Equal("li", RecipientTokens.FieldText([], "li"));
        Assert.Equal(string.Empty, RecipientTokens.FieldText([], string.Empty));
    }

    // A pre-filled field has nothing in progress. This is the reply-all bug: the composer handed
    // the field the core's "a, b" verbatim, and the trailing-token rule, a guess about what the
    // user is typing, read the last address as half-typed. So the field drew one pill and a loose
    // address, and a Cc holding a single recipient drew no pill at all.
    [Fact]
    public void EverySeededRecipientIsFinished()
    {
        var to = RecipientTokens.Seeded("bestuur@example.test, tc@example.test");
        Assert.Equal(
            new[] { "bestuur@example.test", "tc@example.test" },
            RecipientTokens.Committed(to));
        Assert.Equal(string.Empty, RecipientTokens.CurrentToken(to));

        var cc = RecipientTokens.Seeded("rene@example.test");
        Assert.Equal(new[] { "rene@example.test" }, RecipientTokens.Committed(cc));
        Assert.Equal(string.Empty, RecipientTokens.CurrentToken(cc));
    }

    // A new message and a forward seed nothing. Send is gated on a non-blank To, so a lone
    // separator here would enable it over a message addressed to nobody. Seeding twice changes
    // nothing, which is what lets the composer compare a field against its opening value.
    [Fact]
    public void SeedingAnEmptyFieldLeavesItEmptyAndSeedingTwiceChangesNothing()
    {
        Assert.Equal(string.Empty, RecipientTokens.Seeded(string.Empty));
        Assert.Equal(string.Empty, RecipientTokens.Seeded("   "));
        var once = RecipientTokens.Seeded("ada@example.test, grace@example.test");
        Assert.Equal(once, RecipientTokens.Seeded(once));
    }

    // The invariant the field's re-seed guard depends on: the token is TRIMMED, so a raw comparison
    // against the input box would re-seed on every space and eat it. Trimming both sides means only
    // a real change of token resets the box.
    [Fact]
    public void TheTokenIsTrimmedSoTypingASpaceIsNotAChangeOfToken()
    {
        const string typed = "John ";
        Assert.Equal("John", RecipientTokens.CurrentToken(RecipientTokens.FieldText([], typed)));
        Assert.Equal(typed.Trim(), RecipientTokens.CurrentToken("ada@example.test, John "));
    }

    [Fact]
    public void NoSuggestionsShowForAnEmptyToken() =>
        Assert.False(RecipientTokens.ShouldShowSuggestions(
            "ada@example.test, ", ["grace@example.test"]));

    [Fact]
    public void NoSuggestionsShowWhenTheCoreReturnedNone() =>
        Assert.False(RecipientTokens.ShouldShowSuggestions("gr", []));

    [Fact]
    public void SuggestionsShowWhileAPartialAddressIsBeingTyped() =>
        Assert.True(RecipientTokens.ShouldShowSuggestions(
            "ada@example.test, gr", ["grace@example.test"]));

    // Once the token IS the suggestion, the user has finished that recipient, a list offering what
    // is already typed just covers the next field. Case-insensitively: addresses are.
    [Theory]
    [InlineData("grace@example.test")]
    [InlineData("Grace@Example.Test")]
    public void TheListClosesOnceTheTokenIsTheSuggestion(string typed) =>
        Assert.False(RecipientTokens.ShouldShowSuggestions(typed, ["grace@example.test"]));

    // Cc/Bcc open collapsed, so a pre-filled one has to force them open: a recipient the sender
    // cannot see is one they cannot remove, and a mail link may name a Bcc
    // (docs/composer-security.md, Gate 12).
    [Theory]
    [InlineData("carol@example.test", "")]
    [InlineData("", "snoop@evil.test")]
    [InlineData("carol@example.test", "snoop@evil.test")]
    public void APreFilledCcOrBccOpensTheRow(string cc, string bcc) =>
        Assert.True(RecipientTokens.RevealsCcBcc(cc, bcc));

    // Whitespace is not an address, so it does not count as pre-filled.
    [Theory]
    [InlineData("", "")]
    [InlineData("  ", " ")]
    public void AnEmptyCcAndBccLeaveTheRowCollapsed(string cc, string bcc) =>
        Assert.False(RecipientTokens.RevealsCcBcc(cc, bcc));
}
