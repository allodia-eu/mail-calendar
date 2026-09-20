// What one of the four writes did, as something the card can turn into a sentence.
//
// A typed answer rather than a string, so the model half stays free of L10n and the words stay in
// one place. The distinction that earns it: a restart has two endings, running again and a page to
// pay on, and only the second says nothing to the person, because what they see next is a browser.
//
// WinUI-free and L10n-free on purpose, like the card's decisions next door: which sentence follows
// which ending is then a test rather than something only a live subscription can show.

using System;
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

    /// <summary>It did not go through, and the reason says which sentence the card owes.</summary>
    Failed,
}

/// <summary>Why a write did not happen, as the sentence to put in front of the person.</summary>
/// <remarks>
/// ⚠️ **A code, never a message from anywhere else.** The core reports a refusal as an
/// <see cref="AllodiaSubscriptionRefusal"/>, and the purchasing contract asks a client to switch on
/// it, because "you have already cancelled" and "that cannot change while a charge is being
/// retried" are different things to say and only the code tells them apart. Describing the
/// exception instead puts a generated field name on screen: UniFFI builds one out of the variant's
/// own fields, so a refusal reads <c>reason=NotSwitchable</c>, and an unreachable service carries
/// no message at all, which <see cref="CoreError"/> then falls back to the type name for.
/// </remarks>
internal enum AllodiaWriteFailure
{
    /// <summary>
    /// Nothing more can be said: an outage, a refused token, an app that is not running, a reason
    /// this build has never heard of. The sentence says only that nothing changed.
    /// </summary>
    Unexplained,

    /// <summary>Cancelling something already cancelled.</summary>
    AlreadyCancelled,

    /// <summary>Some source is already charging for this account.</summary>
    AlreadyActive,

    /// <summary>Already on the period they asked to move to.</summary>
    AlreadyOnInterval,

    /// <summary>A charge is mid-retry, so the period cannot move underneath it.</summary>
    NotSwitchable,

    /// <summary>There is no such subscription.</summary>
    NotFound,
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
/// <param name="Failure">For <see cref="AllodiaWriteOutcome.Failed"/>: which sentence it earns.</param>
internal sealed record AllodiaWriteResult(
    AllodiaWriteOutcome Outcome,
    string? EndDate = null,
    AllodiaIntervalChange? Change = null,
    AllodiaWriteFailure? Failure = null)
{
    internal static AllodiaWriteResult Cancelled(string endDate) =>
        new(AllodiaWriteOutcome.Cancelled, EndDate: endDate);

    internal static AllodiaWriteResult Reactivated() => new(AllodiaWriteOutcome.Reactivated);

    internal static AllodiaWriteResult Switched(AllodiaIntervalChange change) =>
        new(AllodiaWriteOutcome.Switched, Change: change);

    internal static AllodiaWriteResult Silent() => new(AllodiaWriteOutcome.Silent);

    internal static AllodiaWriteResult Failed(AllodiaWriteFailure failure) =>
        new(AllodiaWriteOutcome.Failed, Failure: failure);

    /// <summary>The sentence <paramref name="error"/> earns.</summary>
    /// <remarks>
    /// Everything that is not a refusal is <see cref="AllodiaWriteFailure.Unexplained"/>, including
    /// a service that could not be reached: nothing was learned, so there is nothing to explain,
    /// and the one thing worth saying is that nothing changed.
    /// </remarks>
    internal static AllodiaWriteFailure FailureFor(Exception error) => error switch
    {
        AllodiaPurchaseException.Refused refused => refused.reason switch
        {
            AllodiaSubscriptionRefusal.AlreadyCancelled => AllodiaWriteFailure.AlreadyCancelled,
            AllodiaSubscriptionRefusal.AlreadyActive => AllodiaWriteFailure.AlreadyActive,
            AllodiaSubscriptionRefusal.AlreadyOnInterval => AllodiaWriteFailure.AlreadyOnInterval,
            AllodiaSubscriptionRefusal.NotSwitchable => AllodiaWriteFailure.NotSwitchable,
            AllodiaSubscriptionRefusal.NotFound => AllodiaWriteFailure.NotFound,
            // Unavailable (no billing on this deployment) and a reason a later service invents both
            // leave a client with nothing specific it could truthfully say.
            _ => AllodiaWriteFailure.Unexplained,
        },
        _ => AllodiaWriteFailure.Unexplained,
    };
}
