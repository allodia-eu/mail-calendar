// What the list header says about how far back a search looked (docs/search.md).
//
// The rule is two branches and a number, which is exactly the kind that regresses without anyone
// noticing: "Searching the last 0 months" over an account that syncs everything is a confident,
// wrong claim, and a horizon rendered on a folder the user merely opened invents a search nobody
// ran. Neither shows up in a screenshot, and the WinUI half cannot be reached from this assembly.
using System;
using Allodia.Mailcal.Services;
using uniffi.mailcal_bindings;
using Xunit;

namespace Allodia.Mailcal.Tests;

public class SearchHorizonLineTests
{
    private const string AllMail = "Searching all mail";

    private static string Line(SearchHorizon? horizon) =>
        SearchHorizonLine.For(horizon, AllMail, months => $"Searching the last {months} months");

    [Fact]
    public void ABoundedDepthNamesItsMonthCount()
    {
        Assert.Equal("Searching the last 3 months", Line(new SearchHorizon.Months(3)));
        Assert.Equal("Searching the last 24 months", Line(new SearchHorizon.Months(24)));
    }

    [Fact]
    public void AnAccountSyncingEverythingSaysSoInsteadOfCountingMonths()
    {
        // The core sends AllTime, never Months(0), but the sentence has to come from the branch
        // and not from the number, or a future zero would read as "the last 0 months".
        Assert.Equal(AllMail, Line(new SearchHorizon.AllTime()));
    }

    [Fact]
    public void AListNobodySearchedStatesNothingAtAll()
    {
        // The strip keys its visibility off the same null the core sends for every non-search
        // list, so an empty string here and a hidden line there are one decision.
        Assert.Equal(string.Empty, Line(null));
    }
}
