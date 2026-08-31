// Which list row is highlighted while the reading pane shows a message, the other half of the
// auto-advance (ReadingAdvance.cs), and the reason it was reported as broken on Windows.
//
// The Apple client derives the highlight from the opened message (`isOpenMessage`), so advancing
// the pane moves the highlight for free. WinUI's ListView instead owns a `SelectedItem`, which
// until now was only ever assigned by the click handlers, so a pane that advanced by itself left
// the highlight behind on a row that had just been archived away. The rule below is what a view
// re-runs whenever the open message *or* the row set changes, so the highlight tracks the pane
// rather than the last click.
//
// **Pure BCL on purpose, no WinUI, no view models.** Same reason as ReadingAdvance: Mailcal.Tests
// links this file and gates the rule on every PR.

using System.Collections.Generic;

namespace Allodia.Mailcal.Services;

/// <summary>
/// One list row as far as the highlight is concerned: which account it belongs to, the message a
/// tap on it opens, and, for a conversation, every message on it, since an expanded thread's
/// sub-rows open those directly.
/// </summary>
public readonly record struct RowStop(
    string Account,
    string LatestKey,
    IReadOnlyList<string> MessageKeys);

/// <summary>Chooses the list row that stands for the message in the reading pane.</summary>
public static class ReadingSelection
{
    /// <summary>
    /// The index in <paramref name="rows"/> of the row that stands for the open message
    /// (<paramref name="account"/> + <paramref name="key"/>), or <c>null</c> when no row does,
    /// the message left the folder, or the list has moved on, in which case nothing is
    /// highlighted.
    /// </summary>
    public static int? RowOf(string account, string key, IReadOnlyList<RowStop> rows)
    {
        for (var i = 0; i < rows.Count; i++)
        {
            // Both halves must match: a provider key is unique only within its account, so two
            // accounts can mint the same one and matching on the key alone highlights a row in
            // the wrong mailbox (the same trap ReadingAdvance guards).
            if (rows[i].Account != account)
            {
                continue;
            }
            if (rows[i].LatestKey == key)
            {
                return i;
            }
            // A conversation row stands for any message on it, the latest one a tap opens, and
            // every other one an expanded thread's sub-rows do.
            foreach (var member in rows[i].MessageKeys)
            {
                if (member == key)
                {
                    return i;
                }
            }
        }

        return null;
    }
}
