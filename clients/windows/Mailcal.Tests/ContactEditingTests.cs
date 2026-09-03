// What the contacts editor turns a form into, and what it refuses (Services/ContactEditing.cs).
//
// The claim worth gating is the one a plausible implementation gets wrong in silence: an edit names
// the CARD it was opened on, and a person is several cards. The rest is in service of it, because
// each is a way to end up saving into the wrong address book, or telling the user to correct a row
// they can see is blank.

using System.Collections.Generic;
using System.Linq;
using Allodia.Mailcal.Services;
using uniffi.mailcal_bindings;
using Xunit;

namespace Allodia.Mailcal.Tests;

public class ContactEditingTests
{
    private static readonly Dictionary<string, string> Labels = new()
    {
        ["personal"] = "me@example.test",
        ["work"] = "me@work.test",
    };

    private static ContactTarget Target(string account, string book, string name, bool isDefault) =>
        new(account, book, name, isDefault);

    private static ContactEdit Edit(
        string given = "",
        string surname = "",
        string organization = "",
        string title = "",
        string[]? emails = null,
        string[]? phones = null) =>
        new(given, surname, organization, title, emails ?? [], phones ?? []);

    /// The picker opens on the account's own default book, not on whichever came first.
    [Fact]
    public void ACreateOpensOnTheDefaultBook()
    {
        var books = ContactEditing.Books(
            [
                Target("personal", "personal-book", "Personal", false),
                Target("work", "work-book", "Work", true),
            ],
            Labels);
        Assert.Equal("work", ContactEditing.DefaultBook(books)?.Account);
    }

    /// A book's own name earns a place only where one account offers several.
    [Fact]
    public void ABookIsLabelledByItsAccountAndByItsNameOnlyWhereThatRepeats()
    {
        var one = ContactEditing.Books(
            [
                Target("personal", "personal-book", "Personal", false),
                Target("work", "work-book", "Work", false),
            ],
            Labels);
        Assert.Equal(["me@example.test", "me@work.test"], one.Select(book => book.Label));

        var several = ContactEditing.Books(
            [
                Target("work", "work-book", "Personal", true),
                Target("work", "team-book", "Team", false),
            ],
            Labels);
        Assert.Equal(
            ["me@work.test (Personal)", "me@work.test (Team)"],
            several.Select(book => book.Label));
    }

    /// A card is labelled by the account the user knows it by, never by the core's internal id.
    [Fact]
    public void ACardChoiceIsLabelledByItsAccount()
    {
        var cards = ContactEditing.Cards(
            [new ContactCardRef("work", "c-work"), new ContactCardRef("unknown", "c-gone")],
            Labels);
        Assert.Equal(["me@work.test", "unknown"], cards.Select(card => card.Label));
    }

    /// A create files into the book the picker chose, with the trimmed form.
    [Fact]
    public void ACreateFilesIntoTheChosenBook()
    {
        var book = new ContactBookChoice("personal", "personal-book", "me@example.test", true);
        var intent = ContactEditing.IntentFor(
            Edit(given: " Grace ", surname: "Hopper", emails: [" grace@example.test "]),
            null,
            book);
        var create = Assert.IsType<Intent.CreateContact>(intent);
        Assert.Equal("personal", create.Account);
        Assert.Equal("personal-book", create.AddressBook);
        Assert.Equal("Grace", create.Edit.GivenName);
        Assert.Equal(["grace@example.test"], create.Edit.Emails);
    }

    /// An edit names the card it was opened on, never the person: a person is several accounts'
    /// cards, and saving without naming one files the work details in the personal book.
    [Fact]
    public void AnEditCarriesTheCardItWasOpenedOn()
    {
        var intent = ContactEditing.IntentFor(
            Edit(given: "Ada", surname: "King", emails: ["ada@example.test"]),
            new EditedCard("7", "work", "c-work"),
            null);
        var update = Assert.IsType<Intent.UpdateContact>(intent);
        Assert.Equal("7", update.Person);
        Assert.Equal("work", update.Account);
        Assert.Equal("c-work", update.Card);
        Assert.Equal("King", update.Edit.Surname);
    }

    /// A company contact has no person's name; a card with none of the three is a blank row.
    [Fact]
    public void AnOrganizationAloneIsEnoughAndNothingAtAllIsNot()
    {
        Assert.Equal(ContactFormError.Empty, ContactEditing.Validate(Edit()));
        Assert.Null(ContactEditing.IntentFor(Edit(), null, null));
        Assert.Null(ContactEditing.Validate(Edit(organization: "Analytical Engines")));
    }

    /// The two refusals are different sentences on screen, so they are different values here.
    [Theory]
    [InlineData("ada")]
    [InlineData("@example.test")]
    [InlineData("ada@")]
    [InlineData("ada@@example.test")]
    [InlineData("ada@.test")]
    public void AMalformedAddressIsItsOwnRefusal(string malformed)
    {
        Assert.Equal(
            ContactFormError.Email,
            ContactEditing.Validate(Edit(given: "Ada", emails: [malformed])));
    }

    /// A row the user emptied is a row they removed: it must not fail validation as a blank
    /// address, and must not reach the core as one.
    [Fact]
    public void BlankRowsAreDroppedRatherThanRefused()
    {
        var edit = Edit(given: "Ada", emails: ["  ", " ada@example.test "], phones: [""]);
        Assert.Null(ContactEditing.Validate(edit));
        var trimmed = ContactEditing.Trim(edit);
        Assert.Equal(["ada@example.test"], trimmed.Emails);
        Assert.Empty(trimmed.Phones);
    }

    /// A contact with no addresses opens with one empty row, so the field is something to type
    /// into rather than a heading over a button.
    [Fact]
    public void AValueFieldOpensOnWhatTheCardHoldsOrOnOneEmptyRow()
    {
        Assert.Equal([string.Empty], ContactEditing.ValueRows([]));
        Assert.Equal(
            ["a@example.test", "b@example.test"],
            ContactEditing.ValueRows(["a@example.test", "b@example.test"]));
    }
}
