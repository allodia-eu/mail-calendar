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
    AllodiaBiller, AllodiaOffer, AllodiaPlan, AllodiaStore, AllodiaStorePage,
    AllodiaSubscriptionView, AllodiaSwitch, ControlState,
};

use super::{
    active_row, cancel_question, free_row, reauth_row, store_page_buttons, switch_question,
    write_buttons,
};
use crate::{
    l10n,
    ui::{
        AppInput,
        allodia_subscription::{SubscriptionInput, SubscriptionWrite},
        controls::tests::spins,
        mailbox::tests::{every_row_belongs_to_a_list, rendered_labels},
    },
};

const FREE: ControlState = ControlState {
    enabled: true,
    running: false,
};

/// What the core decides for a paid account of Allodia's own that permits nothing.
fn paid() -> AllodiaSubscriptionView {
    AllodiaSubscriptionView {
        entitled: true,
        billed_by: Some(AllodiaBiller::Allodia),
        period_end: Some("2026-11-01T00:00:00+00:00".to_owned()),
        renews: true,
        grace: false,
        duplicate_billing: Vec::new(),
        store_pages: Vec::new(),
        paid_through: Some("2026-11-01T00:00:00+00:00".to_owned()),
        switch: None,
        resubscribe: None,
        cancel: None,
        checkout: None,
        refreshing: false,
    }
}

fn offer(plan: AllodiaPlan, price: &str) -> (AllodiaOffer, ControlState) {
    (
        AllodiaOffer {
            plan,
            store: AllodiaStore::Allodia,
            display_price: price.to_owned(),
        },
        FREE,
    )
}

fn window() -> gtk::Window {
    gtk::Window::new()
}

fn labels(widget: &impl IsA<gtk::Widget>) -> Vec<String> {
    rendered_labels(widget.as_ref())
}

/// What each button reads, from the label it renders: a control component's button holds its
/// label as a child beside its spinner.
fn button_labels(widget: &impl IsA<gtk::Widget>) -> Vec<String> {
    buttons(widget).iter().map(button_label).collect()
}

fn button_label(button: &gtk::Button) -> String {
    rendered_labels(button.upcast_ref()).join(" ")
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
    let row = active_row(&sender, &window(), &paid());
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
    let mut answer = paid();
    answer.billed_by = Some(AllodiaBiller::Google);
    answer.store_pages = vec![AllodiaStorePage {
        store: AllodiaStore::Google,
        url: "https://example.test/manage".to_owned(),
    }];

    let row = active_row(&sender, &window(), &answer);
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

/// ⚠️ **The core decides which writes exist, never this file**: a write drawn where it is refused
/// is a button whose only outcome is an error.
pub(crate) fn no_write_is_drawn_that_the_service_has_not_permitted() {
    let (sender, _receiver) = relm4::channel();
    assert!(
        write_buttons(&sender, &window(), &paid()).is_empty(),
        "nothing is permitted, so nothing is drawn"
    );

    let shown: Vec<String> = write_buttons(&sender, &window(), &permitting())
        .iter()
        .map(button_label)
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
    let answer = permitting();

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
    write_buttons(&sender, &window(), &answer)[1].emit_clicked();
    assert!(matches!(
        receiver.recv_sync(),
        Some(AppInput::AllodiaSubscription(SubscriptionInput::Write(
            SubscriptionWrite::Resubscribe
        )))
    ));
}

/// While a write runs its button spins and every button that would start a second one is held:
/// two writes racing each other is two decisions about one person's money.
pub(crate) fn a_write_in_flight_stops_a_second_one() {
    let (sender, _receiver) = relm4::channel();
    let held = ControlState {
        enabled: false,
        running: false,
    };
    let mut answer = permitting();
    answer.cancel = Some(ControlState {
        enabled: false,
        running: true,
    });
    answer.resubscribe = Some(held);
    answer.switch = Some(AllodiaSwitch {
        to: AllodiaPlan::Yearly,
        control: held,
    });

    let drawn = write_buttons(&sender, &window(), &answer);
    for button in &drawn {
        assert!(!button.is_sensitive(), "{}", button_label(button));
    }
    assert!(spins(&drawn[2]), "the cancellation is the work running");
    assert!(!spins(&drawn[0]) && !spins(&drawn[1]));
    for button in write_buttons(&sender, &window(), &permitting()) {
        assert!(button.is_sensitive(), "{}", button_label(&button));
    }
}

/// A cancelled subscription of Allodia's own whose service permits all three writes.
fn permitting() -> AllodiaSubscriptionView {
    AllodiaSubscriptionView {
        renews: false,
        switch: Some(AllodiaSwitch {
            to: AllodiaPlan::Yearly,
            control: FREE,
        }),
        resubscribe: Some(FREE),
        cancel: Some(FREE),
        ..paid()
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

    let row = free_row(&sender, &offers);
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

/// ⚠️ A store holding somebody without access (on hold, paused) still gets its page on the free
/// half: that is where a failed card is replaced and a pause lifted.
pub(crate) fn a_store_page_is_offered_on_the_free_half() {
    let (sender, _receiver) = relm4::channel();
    let held = AllodiaSubscriptionView {
        entitled: false,
        billed_by: None,
        store_pages: vec![AllodiaStorePage {
            store: AllodiaStore::Google,
            url: "https://example.test/manage".to_owned(),
        }],
        ..paid()
    };
    let shown: Vec<String> = store_page_buttons(&sender, &held)
        .iter()
        .map(button_label)
        .collect();
    assert_eq!(
        shown,
        vec![l10n::settings_subscription_manage("Google Play")]
    );
}

/// A shop with nothing to sell draws no button and no small print: the terms belong to an offer,
/// and stating them under nothing claims a purchase is available when it is not.
pub(crate) fn an_account_that_cannot_buy_is_offered_nothing() {
    let (sender, _receiver) = relm4::channel();
    let row = free_row(&sender, &[]);
    assert_eq!(button_labels(&row), [] as [String; 0]);
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
    let section = adw::PreferencesGroup::new();
    section.add(&active_row(&sender, &window(), &permitting()));
    section.add(&free_row(
        &sender,
        &[offer(AllodiaPlan::Yearly, "49,00 EUR")],
    ));
    section.add(&reauth_row(&sender));
    every_row_belongs_to_a_list(section.upcast_ref::<gtk::Widget>());
}
