//! What the subscription section draws in each of its states, and what it must never draw.
//!
//! Rendering is asserted on the **rendered** labels rather than on `ActionRow::title()`, which
//! reads back the string it was handed whatever became of the label; see
//! [`crate::ui::mailbox::tests::rendered_labels`]. Called from the crate's single `gtk::init` test.
//!
//! Every piece here takes what it needs rather than a whole `PageContext`, the shape the account
//! card next door uses: a context cannot be built without an app, and none of these need one.
//!
//! The dates below are English because that is the locale a test run resolves to; the localised
//! forms are pinned next to the formatter itself, in `timestamps`.

use adw::prelude::*;
use mailcal_bindings::{
    AllodiaOffer, AllodiaOwnStatus, AllodiaOwnSubscription, AllodiaPlan, AllodiaPrices,
    AllodiaStore, AllodiaStoreStatus, AllodiaStoreSubscription, AllodiaSubscription,
    AllodiaSubscriptionActions,
};

use super::{active_row, cancel_question, free_row, reauth_row, switch_question, write_buttons};
use crate::{
    l10n,
    ui::{
        AppInput,
        allodia_subscription::{SubscriptionInput, SubscriptionWrite},
        mailbox::tests::{every_row_belongs_to_a_list, rendered_labels},
    },
};

fn subscription(entitled: bool) -> AllodiaSubscription {
    AllodiaSubscription {
        entitled,
        plan: if entitled { "paid" } else { "free" }.to_owned(),
        current_period_end: Some("2026-11-01T00:00:00+00:00".to_owned()),
        own: None,
        stores: Vec::new(),
        duplicate_billing: Vec::new(),
        actions: AllodiaSubscriptionActions {
            can_cancel: false,
            can_resubscribe: false,
            can_switch_interval: false,
            can_start_checkout: true,
        },
        prices: AllodiaPrices {
            monthly_in_cents: 499,
            yearly_in_cents: 4900,
            currency: "EUR".to_owned(),
        },
        checkout_available: true,
    }
}

fn own(status: AllodiaOwnStatus, interval: AllodiaPlan) -> AllodiaOwnSubscription {
    AllodiaOwnSubscription {
        status,
        interval,
        amount_in_cents: 499,
        next_payment_date: Some("2026-11-01T00:00:00+00:00".to_owned()),
        current_period_end: Some("2026-11-01T00:00:00+00:00".to_owned()),
        cancelled_at: None,
    }
}

fn store(status: AllodiaStoreStatus) -> AllodiaStoreSubscription {
    AllodiaStoreSubscription {
        source: AllodiaStore::Google,
        status,
        interval: AllodiaPlan::Monthly,
        product_id: None,
        price_in_cents: None,
        currency: None,
        current_period_end: None,
        auto_renewing: true,
        environment: "production".to_owned(),
        manage_url: "https://example.test/manage".to_owned(),
    }
}

fn offer(plan: AllodiaPlan, price: &str) -> AllodiaOffer {
    AllodiaOffer {
        plan,
        store: AllodiaStore::Allodia,
        display_price: price.to_owned(),
    }
}

fn window() -> gtk::Window {
    gtk::Window::new()
}

fn labels(widget: &impl IsA<gtk::Widget>) -> Vec<String> {
    rendered_labels(widget.as_ref())
}

fn button_labels(widget: &impl IsA<gtk::Widget>) -> Vec<String> {
    let mut found = Vec::new();
    collect_buttons(widget.as_ref(), &mut found);
    found
        .iter()
        .filter_map(gtk::Button::label)
        .map(|label| label.to_string())
        .collect()
}

fn buttons(widget: &impl IsA<gtk::Widget>) -> Vec<gtk::Button> {
    let mut found = Vec::new();
    collect_buttons(widget.as_ref(), &mut found);
    found
}

fn collect_buttons(widget: &gtk::Widget, found: &mut Vec<gtk::Button>) {
    if let Ok(button) = widget.clone().downcast::<gtk::Button>() {
        found.push(button);
    }
    let mut child = widget.first_child();
    while let Some(current) = child {
        collect_buttons(&current, found);
        child = current.next_sibling();
    }
}

/// A paid account says who is charging and until when, and a renewing one is not described as
/// running out. The two sentences are the difference between a charge somebody expects and one
/// they do not.
pub(crate) fn a_paid_account_names_its_biller_and_says_when() {
    let (sender, _receiver) = relm4::channel();
    let mut answer = subscription(true);
    answer.own = Some(own(AllodiaOwnStatus::Active, AllodiaPlan::Monthly));

    let row = active_row(&sender, &window(), &answer, false);
    let shown = labels(&row);
    assert!(
        shown.iter().any(|label| label.contains("Allodia")),
        "the biller has to be named: {shown:?}"
    );
    assert!(
        shown.iter().any(|label| label.contains("1 Nov 2026")),
        "the day it renews on has to be there: {shown:?}"
    );
    assert!(
        shown
            .iter()
            .any(|label| label == &l10n::settings_subscription_renews("1 Nov 2026")),
        "a renewing subscription must not be described as running out: {shown:?}"
    );
}

/// ⚠️ A store's subscription is changed at that store and nowhere else, so the only thing offered
/// for one is its own page. A cancel button here would have nothing to call, and sending somebody
/// to Allodia's page to cancel a store's subscription is how a cancellation quietly does not
/// happen.
pub(crate) fn a_stores_subscription_is_offered_its_page_and_no_write() {
    let (sender, receiver) = relm4::channel();
    let mut answer = subscription(true);
    answer.stores = vec![store(AllodiaStoreStatus::Active)];

    let row = active_row(&sender, &window(), &answer, false);
    let shown = button_labels(&row);
    assert_eq!(
        shown,
        vec![l10n::settings_subscription_manage("Google Play")],
        "one button, and it opens the store's own page"
    );
    buttons(&row)[0].emit_clicked();
    assert!(matches!(
        receiver.recv_sync(),
        Some(AppInput::AllodiaSubscription(
            SubscriptionInput::OpenStorePage(_)
        ))
    ));
}

/// ⚠️ **`actions` decides, never this file.** The service computed it with every biller in view,
/// which no client is in a position to do: a write drawn where it is refused is a button whose only
/// outcome is an error.
pub(crate) fn no_write_is_drawn_that_the_service_has_not_permitted() {
    let (sender, _receiver) = relm4::channel();
    let mut answer = subscription(true);
    answer.own = Some(own(AllodiaOwnStatus::Cancelled, AllodiaPlan::Monthly));

    assert!(
        write_buttons(&sender, &window(), &answer, false).is_empty(),
        "nothing is permitted, so nothing is drawn"
    );

    answer.actions.can_cancel = true;
    answer.actions.can_resubscribe = true;
    answer.actions.can_switch_interval = true;
    let shown: Vec<String> = write_buttons(&sender, &window(), &answer, false)
        .iter()
        .filter_map(gtk::Button::label)
        .map(|label| label.to_string())
        .collect();
    assert_eq!(
        shown,
        vec![
            l10n::settings_subscription_switch_yearly().to_owned(),
            l10n::settings_subscription_resubscribe().to_owned(),
            l10n::settings_subscription_cancel().to_owned(),
        ],
        "the period being moved TO is named, and all three appear once permitted"
    );
}

/// ⚠️ Both of the two that change what somebody is charged ask first, because neither says
/// anything anywhere else until a date that may be a month away. A restart does not: it takes
/// nothing and costs nothing until the next ordinary charge, so it acts on the press.
///
/// The questions are read rather than shown. Presenting an `AdwDialog` needs a window that is on
/// screen, which a unit test has none of; what has to be right is what each one says, and that is
/// here.
pub(crate) fn the_two_writes_that_change_a_charge_ask_before_they_act() {
    let (sender, receiver) = relm4::channel();
    let mut answer = subscription(true);
    answer.own = Some(own(AllodiaOwnStatus::Cancelled, AllodiaPlan::Monthly));
    answer.actions.can_cancel = true;
    answer.actions.can_resubscribe = true;
    answer.actions.can_switch_interval = true;

    let cancelling = cancel_question(&answer);
    // ⚠️ The date is the whole reassurance, and a cancellation with no date reads as "it stops
    // now", which is the one thing it does not do.
    assert!(
        cancelling.body.contains("1 Nov 2026"),
        "{}",
        cancelling.body
    );
    // **Never "Cancel" for the way out.** On this question that word is the thing being asked
    // about, so the answer that does nothing has to say what it keeps.
    assert_eq!(cancelling.keep, l10n::settings_subscription_cancel_keep());
    assert_ne!(cancelling.keep, l10n::action_cancel());
    assert!(cancelling.destructive);

    // ⚠️ No price in the switch question: what this subscriber is charged is not today's list
    // price, and the amount comes back from the switch itself.
    let switching = switch_question(&answer);
    assert!(!switching.body.contains("4,99"), "{}", switching.body);
    assert!(!switching.body.contains("EUR"), "{}", switching.body);
    assert!(switching.body.contains("1 Nov 2026"), "{}", switching.body);

    // The restart asks nothing and writes on the press.
    write_buttons(&sender, &window(), &answer, false)[1].emit_clicked();
    assert!(matches!(
        receiver.recv_sync(),
        Some(AppInput::AllodiaSubscription(SubscriptionInput::Write(
            SubscriptionWrite::Resubscribe
        )))
    ));
}

/// While a write is outstanding every button that would start a second one is insensitive: two
/// writes racing each other is two decisions about one person's money.
pub(crate) fn a_write_in_flight_stops_a_second_one() {
    let (sender, _receiver) = relm4::channel();
    let mut answer = subscription(true);
    answer.own = Some(own(AllodiaOwnStatus::Active, AllodiaPlan::Monthly));
    answer.actions.can_cancel = true;

    for button in write_buttons(&sender, &window(), &answer, true) {
        assert!(!button.is_sensitive(), "{:?}", button.label());
    }
    for button in write_buttons(&sender, &window(), &answer, false) {
        assert!(button.is_sensitive(), "{:?}", button.label());
    }
}

/// **The period is on the button, not under it**, and what renewing means sits beside it rather
/// than a click away: somebody agreeing to a recurring charge is owed both.
pub(crate) fn the_buy_buttons_name_their_period_and_their_price() {
    let (sender, receiver) = relm4::channel();
    let offers = [
        offer(AllodiaPlan::Yearly, "49,00 EUR"),
        offer(AllodiaPlan::Monthly, "4,99 EUR"),
    ];

    let row = free_row(&sender, &offers, false);
    let shown = button_labels(&row);
    assert_eq!(
        shown,
        vec![
            l10n::settings_subscription_buy_yearly("49,00 EUR"),
            l10n::settings_subscription_buy_monthly("4,99 EUR"),
        ],
        "each button names the period it buys and the price it costs"
    );
    assert_eq!(
        row.subtitle().map(|subtitle| subtitle.to_string()),
        Some(l10n::settings_subscription_terms().to_owned()),
        "what renewing means belongs beside the button"
    );
    buttons(&row)[0].emit_clicked();
    assert!(matches!(
        receiver.recv_sync(),
        Some(AppInput::AllodiaSubscription(SubscriptionInput::Write(
            SubscriptionWrite::Checkout(AllodiaPlan::Yearly)
        )))
    ));
}

/// A shop with nothing to sell draws no button and no small print: the terms belong to an offer,
/// and stating them under nothing claims a purchase is available when it is not.
pub(crate) fn an_account_that_cannot_buy_is_offered_nothing() {
    let (sender, _receiver) = relm4::channel();
    let row = free_row(&sender, &[], false);
    assert!(button_labels(&row).is_empty());
    // An untouched row answers with the empty string rather than nothing, which is the same
    // absence and the shape the account card next door already asserts on.
    assert!(row.subtitle().is_none_or(|subtitle| subtitle.is_empty()));
}

/// A grant older than the permission the read needs is an offer, not an error: they are signed in,
/// one thing is asleep, and the ordinary sign-in asks for the full current scope set.
pub(crate) fn a_grant_that_predates_the_read_offers_the_one_thing_that_fixes_it() {
    let (sender, receiver) = relm4::channel();
    let row = reauth_row(&sender);
    assert!(
        labels(&row)
            .iter()
            .any(|label| label == l10n::settings_subscription_reauth()),
        "the row says what is asleep"
    );
    buttons(&row)[0].emit_clicked();
    assert!(matches!(
        receiver.recv_sync(),
        Some(AppInput::StartAllodiaSignIn)
    ));
}

/// Every row this section draws is a list row, which is what makes it reachable from the keyboard
/// and announced by a screen reader; an `AdwActionRow` outside a list is neither.
pub(crate) fn every_subscription_row_is_reachable_from_the_keyboard() {
    let (sender, _receiver) = relm4::channel();
    let mut answer = subscription(true);
    answer.own = Some(own(AllodiaOwnStatus::Active, AllodiaPlan::Monthly));
    answer.actions.can_cancel = true;

    let section = adw::PreferencesGroup::new();
    section.add(&active_row(&sender, &window(), &answer, false));
    section.add(&free_row(
        &sender,
        &[offer(AllodiaPlan::Yearly, "49,00 EUR")],
        false,
    ));
    section.add(&reauth_row(&sender));
    every_row_belongs_to_a_list(section.upcast_ref::<gtk::Widget>());
}
