// The A–Z grouping of the contacts list, the merge-disclosure gate, and the account labelling the
// detail pane's provenance rides on.
//
// The core decides which section each person files under (`ContactRow.Section`), including that an
// accented letter is a letter, and that everything else collects in one `#` bucket rather than each
// symbol minting its own. This turns that flat ordered list into the headers the list draws, and
// does nothing else: no sorting, no re-bucketing, no matching. All three live in the core precisely
// so that four clients cannot disagree about them.
//
// Plain BCL (the generated FFI records are data, and Mailcal.Tests links them), so every rule here
// is a check that can fail rather than a comment in a view.

using System.Collections.Generic;
using System.Linq;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

/// <summary>What the contacts list column has to show.</summary>
internal enum ContactsListState
{
    /// <summary>People to render.</summary>
    Rows,

    /// <summary>Nothing has synced yet (no search is narrowing it).</summary>
    NoContacts,

    /// <summary>A search is active and matched nobody.</summary>
    NoResults,
}

/// <summary>The contacts list's pure decisions: section headers, the merge disclosure, provenance labels.</summary>
internal static class ContactSections
{
    /// <summary>
    /// The A–Z header to draw above the row at <paramref name="index"/>, or <c>null</c> when the
    /// previous row already carries it.
    /// </summary>
    /// <remarks>
    /// Compares with the PREVIOUS row rather than re-bucketing by key: the core hands back one flat
    /// ordered list, and re-grouping it here would be a second ordering that could disagree with the
    /// first, the same reason the Android list decides a header by comparing with the row above.
    /// Order in, order out.
    /// </remarks>
    internal static string? HeaderFor(IReadOnlyList<ContactRow> rows, int index)
    {
        if (index < 0 || index >= rows.Count)
        {
            return null;
        }
        if (index == 0)
        {
            return rows[index].Section;
        }
        return rows[index - 1].Section == rows[index].Section ? null : rows[index].Section;
    }

    /// <summary>
    /// Whether a row must disclose that it is a merge ("In N accounts").
    /// </summary>
    /// <remarks>
    /// Only above one. "In 1 accounts" on every ordinary contact is noise, and ungrammatical noise,
    /// docs/contacts.md §1. The count is of distinct <em>accounts</em>, which the core already
    /// collapsed to; a client must not recount from the source cards.
    /// </remarks>
    internal static bool DisclosesAccounts(uint accountCount) => accountCount > 1;

    /// <summary>
    /// Whether the whole person spans several accounts, so the detail pane's per-value provenance
    /// tags are worth showing.
    /// </summary>
    /// <remarks>
    /// With only one account there is nothing to disambiguate, so the tags are suppressed rather
    /// than repeating the same account name down the screen.
    /// </remarks>
    internal static bool SpansSeveralAccounts(ContactDetail detail) => detail.Accounts.Length > 1;

    /// <summary>
    /// The address the user knows account <paramref name="id"/> by, falling back to the id itself.
    /// </summary>
    /// <remarks>
    /// The core's ids are internal (<c>alice@test.local@jmap:127.0.0.1:18080</c>); showing one is
    /// both ugly and a leak of how ids are built. An id whose account has since been removed falls
    /// back to itself rather than vanishing, a value with no visible source is worse than an ugly one.
    /// </remarks>
    internal static string AccountLabel(string id, IReadOnlyDictionary<string, string> labels) =>
        labels.TryGetValue(id, out var label) ? label : id;

    /// <summary>The accounts carrying one value, joined for a provenance caption.</summary>
    internal static string AccountLabels(
        IReadOnlyList<string> ids,
        IReadOnlyDictionary<string, string> labels) =>
        string.Join(", ", ids.Select(id => AccountLabel(id, labels)));

    /// <summary>
    /// Which of the list column's three states to show.
    /// </summary>
    /// <remarks>
    /// The two empty states are deliberately different sentences: telling someone who has just
    /// searched "No contacts yet" reads as though theirs had vanished.
    /// </remarks>
    internal static ContactsListState ListState(int rowCount, string query)
    {
        if (rowCount > 0)
        {
            return ContactsListState.Rows;
        }
        return string.IsNullOrWhiteSpace(query) ? ContactsListState.NoContacts : ContactsListState.NoResults;
    }
}
