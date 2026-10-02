// Why the mail list has no rows, as the words drawn over it read.
//
// The badge beside the folder counts what the server holds over all time, while the list can only
// show what sync depth kept (docs/folder-pane.md, rule 5). A folder of older mail then badges its
// unread above nothing at all, and without this nothing on screen reconciles the two numbers.
//
// WinUI-free AND L10n-free on purpose, the same shape as SearchHorizonLine: the words are passed
// in, which is what lets Mailcal.Tests link this file and fail on the rule, the mapping from the
// core's enum to the sentence, without a Windows host.

using System;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

internal static class EmptyMailboxLine
{
    /// <summary>
    /// The headline for <paramref name="reason"/>, or the empty string when the list has rows
    /// (the core leaves it unset, and nothing is drawn).
    /// </summary>
    /// <param name="noMail">The words for a folder this device holds whole, and that is empty.</param>
    /// <param name="lastMonths">The words for a depth-bounded list, given its month count.</param>
    internal static string For(EmptyReason? reason, string noMail, Func<int, string> lastMonths) =>
        reason switch
        {
            EmptyReason.NoMail => noMail,
            EmptyReason.OutsideSyncDepth months => lastMonths((int)months.Months),
            _ => string.Empty,
        };

    /// <summary>
    /// Whether the depth setting is worth offering against <paramref name="reason"/>.
    ///
    /// Only where widening would actually find something: on <c>NoMail</c> the device already has
    /// the whole folder, and a button there would promise mail that does not exist.
    /// </summary>
    internal static bool OffersSyncDepth(EmptyReason? reason) =>
        reason is EmptyReason.OutsideSyncDepth;
}
