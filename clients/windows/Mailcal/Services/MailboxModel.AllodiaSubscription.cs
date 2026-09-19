// The subscription half of the Allodia account: the one read that answers the whole card, and the
// four writes against the subscription Allodia bills directly.
//
// The rules are `purchasing.md`, the contract beside the Allodia Licence, and the core holds every
// one of them. What is here is the hop off the UI thread that each blocking core call takes, and
// nothing else: which buttons exist is `Actions`, read where they are drawn, and what a write did
// is the next read's answer rather than this side's guess.
//
// ⚠️ **None of this gates a capability.** `AllodiaEntitlement` does, locally and without a network
// call. This read is what the card *says*, so a service that cannot be reached costs somebody a
// sentence, never access they have paid for.
//
// Windows reaches Allodia's own checkout and no store: the Microsoft Store's commerce is not used,
// so there is no purchase to collect, nothing to acknowledge and no redemption pass to run. The
// checkout is a link, which is what `purchasing.md` says this platform is free to draw.

using System;
using System.Globalization;
using System.Threading.Tasks;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

public sealed partial class MailboxModel
{
    /// <summary>
    /// Everything the subscription card draws, or the state to draw instead when the read did not
    /// come back.
    /// </summary>
    /// <remarks>
    /// The read is a network round trip and the core call blocks on it, so it goes off the UI
    /// thread exactly as the sign-in and sync passes do. A failure is classified from
    /// <see cref="AllodiaGrantHealth"/> rather than from its own text, because a sign-in older
    /// than the permission this needs is an offer with a remedy and an outage is not.
    /// </remarks>
    internal async Task<AllodiaSubscriptionCardState> ReadAllodiaSubscriptionAsync()
    {
        if (_app is null)
        {
            return new AllodiaSubscriptionCardState(AllodiaSubscriptionStatus.Unavailable);
        }
        try
        {
            var answer = await Task.Run(() => _app!.AllodiaSubscription());
            return new AllodiaSubscriptionCardState(AllodiaSubscriptionStatus.Loaded, answer);
        }
        catch (Exception ex)
        {
            // Never who, and never what they pay: a log line describes the user's mail and names
            // no address (docs/logging.md), and this file is what a support request arrives with.
            Log.Info($"allodia: the subscription read did not come back ({CoreError.Describe(ex)})");
            return new AllodiaSubscriptionCardState(
                AllodiaSubscriptionCard.ReadFailure(AllodiaGrantHealth));
        }
    }

    /// <summary>
    /// The two periods this build can sell, priced, <b>in the core's order</b>.
    /// </summary>
    /// <remarks>
    /// ⚠️ <b>The order is the core's and not this client's.</b> Yearly is drawn first because it is
    /// the cheaper rate per month, and `allodia_license::ordered` decides that for every client
    /// precisely so five of them cannot each sort the list their own way. Sorting here would be a
    /// sixth copy of a rule that already has one home.
    /// <para>
    /// The prices arrive as minor units and an ISO 4217 code because the core carries no locale
    /// data, so the formatting is this side's job, exactly as a store's own formatted string is the
    /// store's on the other route. Windows has no other route.
    /// </para>
    /// </remarks>
    internal AllodiaOffer[] AllodiaOffers(AllodiaPrices prices, CultureInfo culture)
    {
        if (_app is null)
        {
            return [];
        }
        AllodiaOffer Offer(AllodiaPlan plan, long minorUnits) => new(
            plan,
            AllodiaStore.Allodia,
            AllodiaSubscriptionFormat.MinorUnits(minorUnits, prices.Currency, culture));
        return _app.OrderAllodiaOffers([
            Offer(AllodiaPlan.Monthly, prices.MonthlyInCents),
            Offer(AllodiaPlan.Yearly, prices.YearlyInCents),
        ]);
    }

    /// <summary>
    /// Opens Allodia's own checkout for <paramref name="plan"/> in the browser, and returns the
    /// failure text when there was nothing to open.
    /// </summary>
    /// <remarks>
    /// ⚠️ <b>The browser, never a web view.</b> The page carries the payment-method choice and the
    /// authorisation for a recurring charge, which is the same reason RFC 8252 keeps an
    /// authorisation request out of one.
    /// <para>
    /// <b>Nothing waits for the result, because there is no result to wait for.</b> The
    /// subscription is created against the account by the checkout itself, so what tells this app
    /// it happened is reading the subscription again, not anything handed back here.
    /// </para>
    /// </remarks>
    internal async Task<string?> StartAllodiaCheckoutAsync(AllodiaPlan plan)
    {
        if (_app is null)
        {
            return "Could not open the app. Please relaunch.";
        }
        try
        {
            var checkout = await Task.Run(() => _app!.StartAllodiaCheckout(plan));
            var failure = await OpenAllodiaCheckoutAsync(checkout);
            // ⚠️ **The one write that records nothing otherwise.** The other three answer with
            // something the card says out loud; this one hands the person to a browser and the app
            // hears nothing further, so without this line a support log cannot tell that a checkout
            // was ever started, and a subscription that appeared from nowhere looks like one. The
            // period only: no address, no amount and not the page, whose URL is a payment session.
            Log.Info(failure is null
                ? $"allodia: a {plan} checkout was opened in the browser"
                : $"allodia: a {plan} checkout could not be opened");
            return failure;
        }
        catch (Exception ex)
        {
            Log.Error($"allodia: a checkout could not be started ({CoreError.Describe(ex)})");
            return CoreError.Describe(ex);
        }
    }

    /// <summary>
    /// Stops the recurring charge, and answers the day access runs to.
    /// </summary>
    /// <remarks>
    /// ⚠️ Only Allodia's own subscription. One bought through a store is cancelled at that store,
    /// and its manage page is where to send the person.
    /// </remarks>
    internal async Task<AllodiaWriteResult> CancelAllodiaSubscriptionAsync()
    {
        if (_app is null)
        {
            return AllodiaWriteResult.Failed("Could not open the app. Please relaunch.");
        }
        try
        {
            var cancelled = await Task.Run(() => _app!.CancelAllodiaSubscription());
            Log.Info("allodia: the subscription was cancelled");
            return AllodiaWriteResult.Cancelled(cancelled.EndDate);
        }
        catch (Exception ex)
        {
            Log.Warn($"allodia: cancel did not go through ({CoreError.Describe(ex)})");
            return AllodiaWriteResult.Failed(CoreError.Describe(ex));
        }
    }

    /// <summary>
    /// Moves between the monthly and the yearly plan, and answers what the next charge becomes.
    /// </summary>
    /// <remarks>
    /// <b>Nothing moves today.</b> The period already paid for runs on untouched, and only the
    /// charge after it changes, which is what the confirmation said and what this reports with the
    /// service's own figures rather than today's list price.
    /// </remarks>
    internal async Task<AllodiaWriteResult> SwitchAllodiaIntervalAsync(AllodiaPlan plan)
    {
        if (_app is null)
        {
            return AllodiaWriteResult.Failed("Could not open the app. Please relaunch.");
        }
        try
        {
            var change = await Task.Run(() => _app!.SwitchAllodiaInterval(plan));
            Log.Info("allodia: the subscription period was changed");
            return AllodiaWriteResult.Switched(change);
        }
        catch (Exception ex)
        {
            Log.Warn($"allodia: the period change did not go through ({CoreError.Describe(ex)})");
            return AllodiaWriteResult.Failed(CoreError.Describe(ex));
        }
    }

    /// <summary>
    /// Starts a cancelled subscription again.
    /// </summary>
    /// <remarks>
    /// Two endings, and the service decides which: restarted on the authorisation already held,
    /// with nothing to pay and nothing to re-enter, or a page to open in a browser. Both are
    /// drawn, which is why <c>Reactivated</c> is read rather than assumed from the absence of a
    /// URL.
    /// </remarks>
    internal async Task<AllodiaWriteResult> ResubscribeToAllodiaAsync()
    {
        if (_app is null)
        {
            return AllodiaWriteResult.Failed("Could not open the app. Please relaunch.");
        }
        try
        {
            var checkout = await Task.Run(() => _app!.ResubscribeToAllodia());
            if (checkout.Reactivated)
            {
                Log.Info("allodia: the subscription is running again");
                return AllodiaWriteResult.Reactivated();
            }
            var failure = await OpenAllodiaCheckoutAsync(checkout);
            return failure is null ? AllodiaWriteResult.Silent() : AllodiaWriteResult.Failed(failure);
        }
        catch (Exception ex)
        {
            Log.Warn($"allodia: the restart did not go through ({CoreError.Describe(ex)})");
            return AllodiaWriteResult.Failed(CoreError.Describe(ex));
        }
    }

    /// <summary>
    /// Opens the store's own subscription page, which is the only thing that can change a
    /// subscription the store is billing.
    /// </summary>
    /// <remarks>
    /// The service sends the page, so this opens what it sent rather than deciding per store:
    /// Windows recognises neither store's own app and has nothing else to offer either way.
    /// </remarks>
    internal void OpenAllodiaStorePage(AllodiaStoreSubscription store)
    {
        if (Uri.TryCreate(store.ManageUrl, UriKind.Absolute, out var url))
        {
            _ = Windows.System.Launcher.LaunchUriAsync(url);
        }
    }

    // The one browser hop both checkout routes share. Returns null once the page is open, and the
    // failure text when the service answered without one, which is an ending neither caller can
    // draw as success.
    private static async Task<string?> OpenAllodiaCheckoutAsync(AllodiaCheckout checkout)
    {
        if (checkout.CheckoutUrl is not { } raw
            || !Uri.TryCreate(raw, UriKind.Absolute, out var url))
        {
            return "The payment page could not be opened.";
        }
        await Windows.System.Launcher.LaunchUriAsync(url);
        return null;
    }
}
