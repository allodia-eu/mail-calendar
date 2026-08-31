// How far back a search looked, as the line under the list header reads it.
//
// Search reads what is on this device and nothing else, so it finds only what sync depth kept
// (docs/search.md). An empty result that does not say so claims "no such message" when it means
// "not in the last three months", and only the second is something the user can fix.
//
// WinUI-free AND L10n-free on purpose: the two words are passed in, the same shape as
// AttendeeSummary's "Organiser". That is what lets Mailcal.Tests link this file and fail on the
// rule, the mapping from the core's enum to the sentence, without a Windows host.

using System;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

internal static class SearchHorizonLine
{
    /// <summary>
    /// The horizon sentence for <paramref name="horizon"/>, or the empty string when the list is
    /// not a search (the core leaves it unset, and the strip renders nothing).
    /// </summary>
    /// <param name="allMail">The words for an account that syncs its whole mailbox.</param>
    /// <param name="lastMonths">The words for a bounded depth, given its month count.</param>
    internal static string For(SearchHorizon? horizon, string allMail, Func<int, string> lastMonths) =>
        horizon switch
        {
            SearchHorizon.AllTime => allMail,
            SearchHorizon.Months months => lastMonths((int)months.MonthsValue),
            _ => string.Empty,
        };
}
