// The subscription half of the Allodia account, which the core reads, keeps and writes
// (`MailcalApp.AllodiaSubscriptionCard`, docs/waiting.md): this sends its intents, opens the pages it
// names, and prices the checkout. Each intent returns at once; the card says what runs until it ends.
//
// Windows sells through Allodia's own checkout and no store: the Microsoft Store's commerce is not
// used, so buying is a page opened in the browser. A store's subscription is read only, and the
// section offers that store's page.

using System;
using System.Globalization;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

public sealed partial class MailboxModel
{
    // The last payment page opened, so each is opened once.
    private ulong _allodiaCheckoutOpened;

    /// <summary>The subscription section as the core decided it; null before the core is up. Local
    /// and cheap; read again on <see cref="AllodiaChanged"/>.</summary>
    internal AllodiaSubscriptionCard? AllodiaSubscriptionCard() => _app?.AllodiaSubscriptionCard();

    /// <summary>Reads the subscription again. Dropped by the core while a read or a write runs,
    /// which answers for it.</summary>
    internal void RefreshAllodiaSubscription() => DispatchAllodia(new AllodiaIntent.RefreshSubscription());

    /// <summary>Buys <paramref name="plan"/> through Allodia's own checkout; the card then names
    /// the payment page, opened on the signal that brings it.</summary>
    internal void StartAllodiaCheckout(AllodiaPlan plan) => DispatchAllodia(new AllodiaIntent.StartCheckout(plan));

    /// <summary>Stops the recurring charge, once the person confirmed.</summary>
    internal void CancelAllodiaSubscription() => DispatchAllodia(new AllodiaIntent.CancelSubscription());

    /// <summary>Starts a cancelled subscription again: restarted at once, or a payment page.</summary>
    internal void ResubscribeToAllodia() => DispatchAllodia(new AllodiaIntent.Resubscribe());

    /// <summary>Moves Allodia's own subscription to <paramref name="plan"/>, once the person
    /// confirmed. Only the next charge changes.</summary>
    internal void SwitchAllodiaInterval(AllodiaPlan plan) => DispatchAllodia(new AllodiaIntent.SwitchInterval(plan));

    /// <summary>
    /// The two periods the checkout sells, priced, <b>in the core's order</b>.
    /// </summary>
    /// <remarks>
    /// The order is `allodia_license::ordered`'s, decided once for every client: yearly first, as
    /// the cheaper rate per month. The prices arrive as minor units and an ISO 4217 code because
    /// the core carries no locale data, so the formatting is this side's job.
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

    /// <summary>Opens a store's own subscription page, the only thing that can change what that
    /// store bills. A page, not a flow: nothing is pending and nothing comes back.</summary>
    internal static void OpenAllodiaStorePage(AllodiaStorePage page)
    {
        if (Uri.TryCreate(page.Url, UriKind.Absolute, out var url))
        {
            _ = ExternalLauncher.Open(url);
        }
    }

    private void DispatchAllodia(AllodiaIntent intent) => _app?.Dispatch(new Intent.Allodia(intent));

    /// <summary>Opens the payment page the last checkout or restart named, once.</summary>
    /// <remarks>
    /// ⚠️ <b>The browser, never a web view</b>: the page carries the payment-method choice and the
    /// authorisation for a recurring charge. Never the URL in a log line: it is a payment session
    /// minted for this person.
    /// </remarks>
    private void OpenAllodiaCheckoutPage()
    {
        if (AllodiaSubscriptionCard() is not { } card
            || AllodiaSubscriptionRules.CheckoutPageToOpen(card, _allodiaCheckoutOpened) is not { } page)
        {
            return;
        }
        _allodiaCheckoutOpened = page.Id;
        if (Uri.TryCreate(page.Url, UriKind.Absolute, out var url))
        {
            _ = ExternalLauncher.Open(url);
        }
        else
        {
            Log.Warn("allodia: the checkout page could not be opened");
        }
    }
}
