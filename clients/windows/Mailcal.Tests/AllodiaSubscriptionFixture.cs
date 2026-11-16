// The subscription section as the core hands it over, built here so each test names only the part
// it is about.

using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Tests;

internal static class AllodiaSubscriptionFixture
{
    internal const string PeriodEnd = "2026-10-11T00:00:00+02:00";

    internal static readonly ControlState Free = new(true, false);

    internal static AllodiaPrices Prices() => new(499, 4999, "EUR");

    /// <summary>What a read decided, renewing on Allodia's own billing unless told otherwise.</summary>
    internal static AllodiaSubscriptionView View(
        bool entitled = true,
        AllodiaBiller[]? duplicate = null,
        AllodiaStorePage[]? storePages = null,
        AllodiaCheckoutOffer? checkout = null,
        bool refreshing = false) => new(
            Entitled: entitled,
            BilledBy: entitled ? new AllodiaBiller.Allodia() : null,
            PeriodEnd: entitled ? PeriodEnd : null,
            Renews: entitled,
            Grace: false,
            DuplicateBilling: duplicate ?? [],
            StorePages: storePages ?? [],
            PaidThrough: entitled ? PeriodEnd : null,
            Switch: entitled ? new AllodiaSwitch(AllodiaPlan.Yearly, Free) : null,
            Resubscribe: null,
            Cancel: entitled ? Free : null,
            Checkout: checkout,
            Refreshing: refreshing);

    internal static AllodiaSubscriptionCard Card(
        AllodiaSubscriptionState state,
        AllodiaSubscriptionOutcome? outcome = null,
        AllodiaCheckoutPage? page = null) => new(state, outcome, page);

    internal static AllodiaSubscriptionCard Loaded(AllodiaSubscriptionView view) =>
        Card(new AllodiaSubscriptionState.Loaded(view));
}
