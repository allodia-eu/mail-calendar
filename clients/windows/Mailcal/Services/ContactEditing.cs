// The contact editor's decisions: which address books a create may offer, which card an edit goes
// to, what the form is refused for, and the intent it becomes.
//
// WinUI-free on purpose, exactly like EventEditorState.cs: the pickers and the dialog live in
// Dialogs/, so what is left here is the part whose failures are silent. Two of them are:
//
//   * An edit that named the PERSON rather than one of their cards would file the work address
//     book's details in the personal one, and the screen would look right (docs/contacts.md §3).
//   * A form that reached the core with one empty address row would be refused as malformed, and
//     the user would be told to check a row they can see is blank.
//
// The validation is a copy of the core's, and deliberately so: the core refuses a card with
// nothing to file it under, but it has no locale and cannot choose the sentence to put under the
// form. The client decides what to say; the core stays the backstop.

using System.Collections.Generic;
using System.Linq;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

/// <summary>Why a contact form cannot be saved.</summary>
internal enum ContactFormError
{
    /// <summary>Nothing to file the card under: no name, no organisation, no address.</summary>
    Empty,

    /// <summary>A value in the address list is not an address.</summary>
    Email,
}

/// <summary>One address book a create can file into, labelled the way the user knows it.</summary>
internal sealed record ContactBookChoice(
    string Account,
    string AddressBook,
    string Label,
    bool IsDefault);

/// <summary>One card an edit could go to, labelled by the account the user knows it by.</summary>
internal sealed record ContactCardChoice(string Account, string Card, string Label);

/// <summary>The card an editor is editing (absent when creating).</summary>
/// <param name="Person">
/// The person the row carried, so a card retired by a merge still resolves.
/// </param>
internal sealed record EditedCard(string Person, string Account, string Card);

/// <summary>The contacts editor's pure decisions.</summary>
internal static class ContactEditing
{
    /// <summary>
    /// Labels every writable book, given the addresses the accounts are known by.
    /// </summary>
    /// <remarks>
    /// The account's address is what a person recognises; the book's own name earns a place only
    /// where one account offers several, where the address alone would repeat down the list.
    /// </remarks>
    internal static IReadOnlyList<ContactBookChoice> Books(
        IReadOnlyList<ContactTarget> targets,
        IReadOnlyDictionary<string, string> accountLabels)
    {
        var perAccount = new Dictionary<string, int>();
        foreach (var target in targets)
        {
            perAccount[target.Account] = perAccount.GetValueOrDefault(target.Account) + 1;
        }
        return [.. targets.Select(target =>
        {
            var account = ContactSections.AccountLabel(target.Account, accountLabels);
            var several = perAccount.GetValueOrDefault(target.Account) > 1;
            return new ContactBookChoice(
                target.Account,
                target.AddressBook,
                several && target.Name.Length > 0 ? $"{account} ({target.Name})" : account,
                target.IsDefault);
        })];
    }

    /// <summary>The book a create opens on: the account's default, else the first on offer.</summary>
    internal static ContactBookChoice? DefaultBook(IReadOnlyList<ContactBookChoice> books) =>
        books.FirstOrDefault(book => book.IsDefault) ?? books.FirstOrDefault();

    /// <summary>The cards an edit could go to, labelled by their accounts.</summary>
    internal static IReadOnlyList<ContactCardChoice> Cards(
        IReadOnlyList<ContactCardRef> cards,
        IReadOnlyDictionary<string, string> accountLabels) =>
        [.. cards.Select(card => new ContactCardChoice(
            card.Account,
            card.Card,
            ContactSections.AccountLabel(card.Account, accountLabels)))];

    /// <summary>
    /// The form with every value trimmed and its blank rows dropped.
    /// </summary>
    /// <remarks>
    /// The core trims too; doing it here as well is what makes <see cref="Validate"/> agree with
    /// the refusal. A form holding one empty address row is a form with no addresses.
    /// </remarks>
    internal static ContactEdit Trim(ContactEdit edit) => new(
        edit.GivenName.Trim(),
        edit.Surname.Trim(),
        edit.Organization.Trim(),
        edit.Title.Trim(),
        [.. edit.Emails.Select(value => value.Trim()).Where(value => value.Length > 0)],
        [.. edit.Phones.Select(value => value.Trim()).Where(value => value.Length > 0)]);

    /// <summary>What is wrong with the form, or <c>null</c> when it can be saved.</summary>
    internal static ContactFormError? Validate(ContactEdit edit)
    {
        var trimmed = Trim(edit);
        if (trimmed.GivenName.Length == 0 && trimmed.Surname.Length == 0
            && trimmed.Organization.Length == 0 && trimmed.Emails.Length == 0)
        {
            return ContactFormError.Empty;
        }
        return trimmed.Emails.Any(value => !IsAddressShaped(value))
            ? ContactFormError.Email
            : null;
    }

    /// <summary>
    /// The intent a Save dispatches, or <c>null</c> when the form is not valid.
    /// </summary>
    /// <param name="editing">The card being edited, or <c>null</c> for a create.</param>
    /// <param name="book">Where a create files the contact; ignored on an edit.</param>
    internal static Intent? IntentFor(ContactEdit edit, EditedCard? editing, ContactBookChoice? book)
    {
        if (Validate(edit) is not null)
        {
            return null;
        }
        var trimmed = Trim(edit);
        return editing is null
            ? new Intent.CreateContact(book?.Account, book?.AddressBook, trimmed)
            : new Intent.UpdateContact(editing.Person, editing.Account, editing.Card, trimmed);
    }

    /// <summary>
    /// Whether a string is shaped like an email address; the same test the core applies.
    /// </summary>
    /// <remarks>
    /// A backstop, not a parser: the server is the authority on what it accepts. What this catches
    /// is the value that would reach it as a malformed card and come back as an opaque 400.
    /// </remarks>
    internal static bool IsAddressShaped(string value)
    {
        var at = value.IndexOf('@');
        if (at <= 0 || at == value.Length - 1)
        {
            return false;
        }
        var domain = value[(at + 1)..];
        return !domain.Contains('@') && !domain.StartsWith('.');
    }

    /// <summary>
    /// The rows a value field opens with: what the card holds, else one empty row.
    /// </summary>
    /// <remarks>
    /// One empty row so the field is something to type in rather than a heading over a button.
    /// </remarks>
    internal static IReadOnlyList<string> ValueRows(IReadOnlyList<string> values) =>
        values.Count == 0 ? [string.Empty] : [.. values];
}
