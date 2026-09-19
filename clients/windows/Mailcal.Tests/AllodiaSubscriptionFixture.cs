// The account-service answers the subscription card reads, built here so each test names only the
// part it is about.
//
// Everything in `purchasing.md` that a card can get wrong is a shape of this record: who is
// billing, whether anything renews, whether a charge is being retried, and what the service says
// the caller may do. A fixture keeps those legible instead of burying each rule under nine fields.

using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Tests;

internal static class AllodiaSubscriptionFixture
{
    internal const string PeriodEnd = "2026-10-11T00:00:00+02:00";

    /// <summary>Nobody may do anything, which is what the service says about a free account.</summary>
    internal static AllodiaSubscriptionActions NoActions() => new(false, false, false, false);

    internal static AllodiaPrices Prices() => new(499, 4999, "EUR");

    /// <summary>A subscription Allodia bills directly, renewing, with nothing else charging.</summary>
    internal static AllodiaSubscription Own(
        AllodiaOwnStatus? status = null,
        AllodiaPlan interval = AllodiaPlan.Monthly,
        string? nextPayment = PeriodEnd,
        AllodiaSubscriptionActions? actions = null) =>
        new(
            Entitled: true,
            Plan: "standard",
            CurrentPeriodEnd: PeriodEnd,
            Own: new AllodiaOwnSubscription(
                Status: status ?? new AllodiaOwnStatus.Active(),
                Interval: interval,
                AmountInCents: 499,
                NextPaymentDate: nextPayment,
                CurrentPeriodEnd: PeriodEnd,
                CancelledAt: null),
            Stores: [],
            DuplicateBilling: [],
            Actions: actions ?? new AllodiaSubscriptionActions(true, false, true, false),
            Prices: Prices(),
            CheckoutAvailable: true);

    /// <summary>A subscription a store is billing, which this API may only read.</summary>
    internal static AllodiaStoreSubscription Store(
        AllodiaStore source,
        AllodiaStoreStatus? status = null,
        bool autoRenewing = true) =>
        new(
            Source: source,
            Status: status ?? new AllodiaStoreStatus.Active(),
            Interval: AllodiaPlan.Monthly,
            ProductId: "eu.allodia.mailcal.monthly",
            PriceInCents: 599,
            Currency: "EUR",
            CurrentPeriodEnd: PeriodEnd,
            AutoRenewing: autoRenewing,
            Environment: "production",
            ManageUrl: "https://example.test/manage");

    /// <summary>An account a store is billing, and nothing of Allodia's own.</summary>
    internal static AllodiaSubscription Stores(
        params AllodiaStoreSubscription[] stores) =>
        new(
            Entitled: true,
            Plan: "standard",
            CurrentPeriodEnd: PeriodEnd,
            Own: null,
            Stores: stores,
            DuplicateBilling: [],
            // Everything false: a store's subscription is changed at that store, and the service is
            // what says so.
            Actions: NoActions(),
            Prices: Prices(),
            CheckoutAvailable: true);

    /// <summary>Nothing is being charged, so both periods are on offer.</summary>
    internal static AllodiaSubscription Free() =>
        new(
            Entitled: false,
            Plan: "free",
            CurrentPeriodEnd: null,
            Own: null,
            Stores: [],
            DuplicateBilling: [],
            Actions: new AllodiaSubscriptionActions(false, false, false, true),
            Prices: Prices(),
            CheckoutAvailable: true);
}
