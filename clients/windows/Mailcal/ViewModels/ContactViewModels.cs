// Public, render-ready contact types the XAML binds to. The generated UniFFI contact records are
// `internal` (and carry lowercase Rust field names), so MailboxModel projects them into these
// public POCOs, the same discipline RowViewModels.cs keeps for the mailbox, so the FFI types stay
// confined to the service layer.
//
// Every localised string and every Visibility is resolved HERE, at projection time, rather than in
// the view: the core owns no locale facility, and the two rules this surface can quietly break,
// "(no name)" for a nameless card, and never "In 1 accounts", are then decided in one place
// instead of once per template.

using System;
using System.Collections.Generic;
using System.Linq;
using Microsoft.UI.Xaml;

namespace Allodia.Mailcal.ViewModels;

/// <summary>One contacts-list row: a unified person, ready to render.</summary>
public sealed class ContactItem
{
    /// <summary>The person's stable id, what opens their detail.</summary>
    public required string Id { get; init; }

    /// <summary>The display name, with the "(no name)" placeholder already substituted.</summary>
    public required string DisplayName { get; init; }

    /// <summary>The address under the name; empty for a person with no email.</summary>
    public required string Email { get; init; }

    /// <summary>
    /// The person's face: their photo where an account's address book has one, else the same
    /// letters on their own colour (docs/avatars.md).
    /// </summary>
    public required AvatarItem Avatar { get; init; }

    /// <summary>The A–Z header drawn above this row, or empty when the row above carries it.</summary>
    public string SectionHeader { get; init; } = string.Empty;

    /// <summary>The merge disclosure ("In 2 accounts"), or empty for an ordinary contact.</summary>
    public string AccountCountText { get; init; } = string.Empty;

    /// <summary>Show the section header only on the first row of a section.</summary>
    public Visibility SectionVisibility =>
        string.IsNullOrEmpty(SectionHeader) ? Visibility.Collapsed : Visibility.Visible;

    /// <summary>Show the address line only when there is one.</summary>
    public Visibility EmailVisibility =>
        string.IsNullOrEmpty(Email) ? Visibility.Collapsed : Visibility.Visible;

    /// <summary>Show the "In N accounts" disclosure only on an actual merge.</summary>
    public Visibility AccountCountVisibility =>
        string.IsNullOrEmpty(AccountCountText) ? Visibility.Collapsed : Visibility.Visible;

    /// <summary>
    /// What a screen reader announces for the row.
    /// </summary>
    /// <remarks>
    /// A WinUI <c>ListViewItem</c> with no accessible name of its own falls back to its data item's
    /// <c>ToString()</c>, which is the type name, so this is overridden rather than left to
    /// announce "Allodia.Mailcal.ViewModels.ContactItem". It is also why the monogram is marked
    /// decorative: it restates the initials of a name that is about to be read, which is only true
    /// if the name is actually read.
    /// </remarks>
    public override string ToString() => string.Join(
        ", ",
        new[] { DisplayName, Email, AccountCountText }.Where(part => !string.IsNullOrEmpty(part)));
}

/// <summary>One value on a contact (an address, phone, organisation or title), with its provenance.</summary>
public sealed class ContactValueItem
{
    /// <summary>The value itself.</summary>
    public required string Value { get; init; }

    /// <summary>The accounts carrying it, already mapped to addresses the user recognises. Empty
    /// when the person came from a single account and there is nothing to disambiguate.</summary>
    public string AccountsText { get; init; } = string.Empty;

    /// <summary>Show the provenance caption only when the person spans several accounts.</summary>
    public Visibility AccountsVisibility =>
        string.IsNullOrEmpty(AccountsText) ? Visibility.Collapsed : Visibility.Visible;
}

/// <summary>One labelled group of values in the detail pane (Email / Phone / Organisation / Role).</summary>
public sealed class ContactValueGroup
{
    /// <summary>The section heading.</summary>
    public required string Heading { get; init; }

    /// <summary>The values under it.</summary>
    public IReadOnlyList<ContactValueItem> Values { get; init; } = Array.Empty<ContactValueItem>();
}

/// <summary>One person's detail: every value the core assembled, and which accounts supplied it.</summary>
public sealed class ContactDetailItem
{
    /// <summary>
    /// The <b>resolved</b> person's id, which need not be the row id it was opened from: merging
    /// retires ids, and the core keeps the retired ones pointing at the surviving person.
    /// </summary>
    public required string Id { get; init; }

    /// <summary>The display name, with the "(no name)" placeholder already substituted.</summary>
    public required string DisplayName { get; init; }

    /// <summary>The person's face, matching the row's, the screen a photo is most of.</summary>
    public required AvatarItem Avatar { get; init; }

    /// <summary>The non-empty value groups, in Email / Phone / Organisation / Role order.</summary>
    public IReadOnlyList<ContactValueGroup> Groups { get; init; } = Array.Empty<ContactValueGroup>();

    /// <summary>
    /// The accounts this person was assembled from, the "Also in" explanation a merged row owes.
    /// Empty for an ordinary contact: naming the single account it came from is noise, and would
    /// make every contact look like a merge.
    /// </summary>
    public IReadOnlyList<string> Accounts { get; init; } = Array.Empty<string>();

    /// <summary>Show the "Also in" section only for an actual merge.</summary>
    public Visibility AccountsVisibility =>
        Accounts.Count == 0 ? Visibility.Collapsed : Visibility.Visible;
}
