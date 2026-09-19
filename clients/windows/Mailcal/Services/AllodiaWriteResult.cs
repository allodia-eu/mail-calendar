// What one of the four writes did, as something the card can turn into a sentence.
//
// A typed answer rather than a string, so the model half stays free of L10n and the words stay in
// one place. The distinction that earns it: a restart has two endings, running again and a page to
// pay on, and only the second says nothing to the person, because what they see next is a browser.
//
// WinUI-free and L10n-free on purpose, like the card's decisions next door: which sentence follows
// which ending is then a test rather than something only a live subscription can show.

using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

/// <summary>Which of the endings a write reached.</summary>
internal enum AllodiaWriteOutcome
{
    /// <summary>The recurring charge was stopped. Access runs to the date.</summary>
    Cancelled,

    /// <summary>
    /// The subscription is running again on the authorisation already held: nothing to pay,
    /// nothing to re-enter.
    /// </summary>
    Reactivated,

    /// <summary>The next charge moved to the other period. Nothing moved today.</summary>
    Switched,

    /// <summary>
    /// Nothing to say here, because what happens next is somewhere else: a payment page is opening
    /// in the browser. The re-read that follows is what will report whatever it leads to.
    /// </summary>
    Silent,

    /// <summary>It did not go through, and the text says what the core called it.</summary>
    Failed,
}

/// <summary>One write's ending, with whichever of the service's figures belongs to it.</summary>
/// <param name="Outcome">Which ending.</param>
/// <param name="EndDate">
/// For <see cref="AllodiaWriteOutcome.Cancelled"/>: the ISO 8601 day access runs to, which is the
/// whole reassurance and the reason a cancellation reports anything at all.
/// </param>
/// <param name="Change">
/// For <see cref="AllodiaWriteOutcome.Switched"/>: what the next charge becomes, and when. ⚠️ The
/// amount arrives as minor units with no currency beside it, so the card prices it from the
/// subscription's own <c>Prices.Currency</c>.
/// </param>
/// <param name="Failure">For <see cref="AllodiaWriteOutcome.Failed"/>: what the core called it.</param>
internal sealed record AllodiaWriteResult(
    AllodiaWriteOutcome Outcome,
    string? EndDate = null,
    AllodiaIntervalChange? Change = null,
    string? Failure = null)
{
    internal static AllodiaWriteResult Cancelled(string endDate) =>
        new(AllodiaWriteOutcome.Cancelled, EndDate: endDate);

    internal static AllodiaWriteResult Reactivated() => new(AllodiaWriteOutcome.Reactivated);

    internal static AllodiaWriteResult Switched(AllodiaIntervalChange change) =>
        new(AllodiaWriteOutcome.Switched, Change: change);

    internal static AllodiaWriteResult Silent() => new(AllodiaWriteOutcome.Silent);

    internal static AllodiaWriteResult Failed(string failure) =>
        new(AllodiaWriteOutcome.Failed, Failure: failure);
}
