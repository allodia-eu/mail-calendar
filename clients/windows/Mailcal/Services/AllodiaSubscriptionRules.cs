// How Settings → Allodia account → Subscription words what the core decided
// (`MailcalApp.AllodiaSubscriptionCard`, docs/waiting.md): who is charging, a store's name, what a
// write came to, and the page to open once. The core decides every state, control and outcome; this
// only names and formats them.
//
// WinUI-free AND L10n-free on purpose: every word arrives in AllodiaSubscriptionWords, so
// Mailcal.Tests can link this file without a Windows host.

using System;
using System.Globalization;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

/// <summary>The sentences a write's outcome is said in, from the catalog in the app and plain in a
/// test.</summary>
internal sealed record AllodiaSubscriptionWords(
    Func<string, string> Cancelled,
    string Restarted,
    Func<string, string, string> Switched,
    string WriteFailed,
    string AlreadyCancelled,
    string AlreadyActive,
    string AlreadyOnInterval,
    string NotSwitchable,
    string NotFound);

internal static class AllodiaSubscriptionRules
{
    /// <summary>What a biller is called on screen.</summary>
    /// <remarks>
    /// <b>Allodia, never the payment processor behind it.</b> Naming a processor somebody has never
    /// heard of, inside a message about being charged twice, is how a correct warning reads as a
    /// scam.
    /// </remarks>
    internal static string BillerName(AllodiaBiller biller) => biller switch
    {
        AllodiaBiller.Apple => "Apple",
        AllodiaBiller.Google => "Google Play",
        // A biller this build cannot name still has to appear, or a "you are being charged twice"
        // warning would name only one of the two.
        AllodiaBiller.Unknown unknown => unknown.Label,
        _ => "Allodia",
    };

    /// <summary>What a store is called on its page's button.</summary>
    internal static string StoreName(AllodiaStore store) => store switch
    {
        AllodiaStore.Apple => "Apple",
        AllodiaStore.Google => "Google Play",
        // The service never reports its own billing as a store.
        _ => "Allodia",
    };

    /// <summary>Whether coming back to the window is worth asking the service again: only where
    /// an answer is drawn. A read still out answers for the return.</summary>
    internal static bool WorthRereading(AllodiaSubscriptionState state) => state
        is AllodiaSubscriptionState.Unavailable
        or AllodiaSubscriptionState.NeedsReauth
        or AllodiaSubscriptionState.Loaded;

    /// <summary>The payment page to open now: one not opened before (<paramref name="opened"/> is
    /// the last id that was).</summary>
    internal static AllodiaCheckoutPage? CheckoutPageToOpen(AllodiaSubscriptionCard card, ulong opened) =>
        card.CheckoutPage is { } page && page.Id > opened ? page : null;

    /// <summary>The control of the buy button for <paramref name="plan"/>.</summary>
    internal static ControlState CheckoutControl(AllodiaCheckoutOffer checkout, AllodiaPlan plan) =>
        plan == AllodiaPlan.Yearly ? checkout.Yearly : checkout.Monthly;

    /// <summary>What a redraw of the section compares.</summary>
    /// <remarks>A record's own text names an array by its type, so the two lists are spelled out:
    /// a store page appearing on its own is still a change to draw.</remarks>
    internal static string Fingerprint(AllodiaSubscriptionCard? card) => card switch
    {
        null => "-",
        { State: AllodiaSubscriptionState.Loaded { View: var view } } =>
            $"{card}/{string.Join(",", view.DuplicateBilling)}/{string.Join(",", view.StorePages)}",
        _ => card.ToString(),
    };

    /// <summary>What a write came to, in <paramref name="culture"/>.</summary>
    internal static string OutcomeText(
        AllodiaSubscriptionOutcome outcome, AllodiaSubscriptionWords words, CultureInfo culture) => outcome switch
    {
        // ⚠️ The date is the whole reassurance; one this build cannot read is still said, since a
        // sentence with no date at all is not true.
        AllodiaSubscriptionOutcome.Cancelled cancelled =>
            words.Cancelled(AllodiaSubscriptionFormat.Date(cancelled.EndDate, culture) ?? cancelled.EndDate),
        AllodiaSubscriptionOutcome.Restarted => words.Restarted,
        // The service's own figures, never today's list price.
        AllodiaSubscriptionOutcome.Switched switched => words.Switched(
            AllodiaSubscriptionFormat.Date(switched.NextPaymentDate, culture) ?? string.Empty,
            AllodiaSubscriptionFormat.MinorUnits(switched.AmountInCents, switched.Currency, culture)),
        AllodiaSubscriptionOutcome.Failed failed => Refusal(failed.Failure, words),
        _ => words.WriteFailed,
    };

    /// <summary>A refusal's own sentence.</summary>
    /// <remarks>⚠️ <b>A code, never a message from anywhere else</b>: "you have already cancelled"
    /// and "that cannot change while a charge is being retried" are different things to say, and
    /// only the code tells them apart.</remarks>
    internal static string Refusal(AllodiaWriteFailure failure, AllodiaSubscriptionWords words) => failure switch
    {
        AllodiaWriteFailure.AlreadyCancelled => words.AlreadyCancelled,
        AllodiaWriteFailure.AlreadyActive => words.AlreadyActive,
        AllodiaWriteFailure.AlreadyOnInterval => words.AlreadyOnInterval,
        AllodiaWriteFailure.NotSwitchable => words.NotSwitchable,
        AllodiaWriteFailure.NotFound => words.NotFound,
        _ => words.WriteFailed,
    };
}
