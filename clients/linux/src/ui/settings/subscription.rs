//! Settings → Allodia account → Subscription: what is being charged, by whom, until when, the way
//! to start one, and the three things somebody may do to the subscription Allodia bills directly.
//!
//! Its Apple, Android and Windows twins are `AllodiaSubscriptionSettings.swift`,
//! `AllodiaSubscriptionCard.kt` and `SettingsDialog.AllodiaSubscription.cs`: keep the states and
//! the wording in step.
//!
//! The rules are `purchasing.md`, the contract beside the Allodia Licence, and the core holds every
//! one of them. **What this file draws is what the core answered.** Which offers exist and in what
//! order is the core's `order_allodia_offers`; `actions` decides which buttons exist at all,
//! because the rule behind it has to account for three billers at once; and a store's subscription
//! is changed only at that store, so its button opens the store's page rather than doing anything.
//!
//! ⚠️ **Linux sells through Allodia's own checkout**, which is a link here: Flatpak has no commerce
//! and there is no programme to enrol in. So the buy buttons price today's list from `prices`, in
//! minor units this side formats, rather than drawing a store's own formatted string.

use adw::prelude::*;
use mailcal_bindings::{AllodiaOffer, AllodiaPlan, AllodiaStore, AllodiaSubscription};

use super::{PageContext, group_titled};
use crate::{
    l10n,
    ui::{
        AppInput,
        allodia_subscription::{SubscriptionAnswer, SubscriptionInput, SubscriptionWrite},
        allodia_subscription_facts as facts,
        mailbox::plain_text_row,
        timestamps,
    },
};

const RESPONSE_KEEP: &str = "keep";
const RESPONSE_GO_AHEAD: &str = "go-ahead";

/// The section, or `None` while nobody is signed in: a subscription belongs to an account, and
/// there is nothing to say about one that does not exist.
pub(super) fn subscription(ctx: &PageContext) -> Option<adw::PreferencesGroup> {
    // A subscription belongs to an account, so nobody signed in is nothing to draw.
    ctx.app.allodia_account()?;
    let state = &ctx.allodia_subscription;
    let section = group_titled(l10n::settings_subscription_heading());
    match &state.answer {
        // Nobody has asked yet, so this build asks now. A network round trip, which is why it
        // waits for somebody to open the page that draws it rather than running at connect.
        None => {
            if !state.checking {
                ctx.sender
                    .emit(AppInput::AllodiaSubscription(SubscriptionInput::Read));
            }
            section.add(&checking_row());
        }
        // Deliberately quiet. A read that did not arrive costs a sentence, never access: whether a
        // capability is on is the entitlement's answer, and it is local.
        Some(SubscriptionAnswer::Unavailable) => {
            section.add(&text_row(l10n::settings_subscription_unavailable()));
        }
        // An offer rather than an error: they are signed in and this one read is asleep, and the
        // ordinary sign-in asks for the full current scope set.
        Some(SubscriptionAnswer::NeedsReauth) => section.add(&reauth_row(&ctx.sender)),
        Some(SubscriptionAnswer::Loaded(subscription)) => loaded(&section, ctx, subscription),
    }
    if let Some(note) = &state.note {
        section.add(&text_row(note));
    }
    Some(section)
}

fn loaded(section: &adw::PreferencesGroup, ctx: &PageContext, subscription: &AllodiaSubscription) {
    let writing = ctx.allodia_subscription.writing;
    if subscription.entitled {
        section.add(&active_row(&ctx.sender, &ctx.window, subscription, writing));
        // Not a lapse, and never drawn as one: the charge is being retried and access continues.
        if facts::in_grace(subscription)
            && let Some(first) = facts::billers(subscription).first()
        {
            section.add(&text_row(&l10n::settings_subscription_grace(
                &facts::biller_name(first),
            )));
        }
        // Reported, never resolved. Cancelling one of them without asking is a decision about
        // somebody else's money, so each exit is offered and none is taken.
        if !subscription.duplicate_billing.is_empty() {
            let names: Vec<String> = subscription
                .duplicate_billing
                .iter()
                .map(facts::biller_name)
                .collect();
            let row = text_row(&l10n::settings_subscription_duplicate(&names.join(", ")));
            row.add_css_class("error");
            section.add(&row);
        }
    } else {
        section.add(&free_row(&ctx.sender, &offers(ctx, subscription), writing));
    }
}

/// What a paid account says: who is charging, until when, and everything that may be done about
/// it, in one row. The shape the account card next door already uses for its three buttons.
fn active_row(
    sender: &relm4::Sender<AppInput>,
    window: &gtk::Window,
    subscription: &AllodiaSubscription,
    writing: bool,
) -> adw::ActionRow {
    let row = plain_text_row();
    let billers = facts::billers(subscription);
    if let Some(first) = billers.first() {
        row.set_title(&l10n::settings_subscription_billed_by(&facts::biller_name(
            first,
        )));
    }
    if let Some(end) = subscription
        .current_period_end
        .as_deref()
        .and_then(|raw| timestamps::account_date(raw, l10n::active_locale()))
    {
        row.set_subtitle(&if facts::will_renew(subscription) {
            l10n::settings_subscription_renews(&end)
        } else {
            l10n::settings_subscription_ends(&end)
        });
    }
    // A store's subscription is the store's to change, so this opens its page rather than offering
    // a cancel button that would have nothing to call. One button per store: an account carries
    // two subscriptions at one store the moment somebody resubscribes there.
    for store in facts::manageable_stores(subscription) {
        let biller = if store.source == AllodiaStore::Apple {
            "Apple"
        } else {
            "Google Play"
        };
        let manage = button(&l10n::settings_subscription_manage(biller));
        let url = store.manage_url.clone();
        let sender = sender.clone();
        manage.connect_clicked(move |_| {
            sender.emit(AppInput::AllodiaSubscription(
                SubscriptionInput::OpenStorePage(url.clone()),
            ));
        });
        row.add_suffix(&manage);
    }
    for write in write_buttons(sender, window, subscription, writing) {
        row.add_suffix(&write);
    }
    row
}

/// What may be done to Allodia's own subscription, which the service decided: a client draws
/// exactly what `actions` permits and works nothing out for itself.
fn write_buttons(
    sender: &relm4::Sender<AppInput>,
    window: &gtk::Window,
    subscription: &AllodiaSubscription,
    writing: bool,
) -> Vec<gtk::Button> {
    let mut buttons = Vec::new();
    if let Some(target) = facts::switch_target(subscription) {
        let label = if target == AllodiaPlan::Yearly {
            l10n::settings_subscription_switch_yearly()
        } else {
            l10n::settings_subscription_switch_monthly()
        };
        buttons.push(asking_button(
            sender,
            window,
            label,
            switch_question(subscription),
            SubscriptionWrite::Switch(target),
        ));
    }
    if facts::offers_restart(subscription) {
        let restart = button(l10n::settings_subscription_resubscribe());
        let sender = sender.clone();
        restart.connect_clicked(move |_| {
            sender.emit(AppInput::AllodiaSubscription(SubscriptionInput::Write(
                SubscriptionWrite::Resubscribe,
            )));
        });
        buttons.push(restart);
    }
    if subscription.actions.can_cancel {
        let cancel = asking_button(
            sender,
            window,
            l10n::settings_subscription_cancel(),
            cancel_question(subscription),
            SubscriptionWrite::Cancel,
        );
        cancel.add_css_class("destructive-action");
        buttons.push(cancel);
    }
    for button in &buttons {
        button.set_sensitive(!writing);
    }
    buttons
}

/// What an account nobody is charging says, and the two periods it may be bought for.
fn free_row(
    sender: &relm4::Sender<AppInput>,
    offers: &[AllodiaOffer],
    writing: bool,
) -> adw::ActionRow {
    let row = text_row(l10n::settings_subscription_free());
    if offers.is_empty() {
        return row;
    }
    // What renewing means, beside the buttons and not a click away: somebody agreeing to a
    // recurring charge is owed it.
    row.set_subtitle(l10n::settings_subscription_terms());
    for offer in offers {
        // **The period is on the button, not under it.** It is the thing being chosen, so a row of
        // buttons that all read "Subscribe" makes the reader pair each one with a line of small
        // print to find out what it does.
        let label = if offer.plan == AllodiaPlan::Yearly {
            l10n::settings_subscription_buy_yearly(&offer.display_price)
        } else {
            l10n::settings_subscription_buy_monthly(&offer.display_price)
        };
        let buy = button(&label);
        buy.add_css_class("suggested-action");
        buy.set_sensitive(!writing);
        let sender = sender.clone();
        let plan = offer.plan;
        buy.connect_clicked(move |_| {
            sender.emit(AppInput::AllodiaSubscription(SubscriptionInput::Write(
                SubscriptionWrite::Checkout(plan),
            )));
        });
        row.add_suffix(&buy);
    }
    row
}

/// The plans this build's shop will sell, in the core's order.
///
/// Empty while something is already being charged: the service refuses a second subscription for
/// one plan, so offering one would be drawing a button whose only outcome is a refusal. That
/// answer is `can_start_checkout`'s and is read rather than worked out here.
fn offers(ctx: &PageContext, subscription: &AllodiaSubscription) -> Vec<AllodiaOffer> {
    if !subscription.checkout_available || !subscription.actions.can_start_checkout {
        return Vec::new();
    }
    let locale = l10n::active_locale();
    let prices = &subscription.prices;
    ctx.app.order_allodia_offers(vec![
        AllodiaOffer {
            plan: AllodiaPlan::Yearly,
            store: AllodiaStore::Allodia,
            display_price: facts::minor_units(prices.yearly_in_cents, &prices.currency, locale),
        },
        AllodiaOffer {
            plan: AllodiaPlan::Monthly,
            store: AllodiaStore::Allodia,
            display_price: facts::minor_units(prices.monthly_in_cents, &prices.currency, locale),
        },
    ])
}

/// What one confirmation asks.
///
/// Both of the questions that use it change what somebody is charged, and neither says so anywhere
/// else: a cancellation is silent until the period runs out, and a switch is silent until a date
/// that may be a month away. So both are asked before they are done.
struct Question {
    title: &'static str,
    body: String,
    go_ahead: &'static str,
    keep: &'static str,
    destructive: bool,
}

/// What stopping the recurring charge asks.
fn cancel_question(subscription: &AllodiaSubscription) -> Question {
    Question {
        title: l10n::settings_subscription_cancel_title(),
        // ⚠️ The date is the whole reassurance: somebody cancelling wants to know they are not
        // losing what they have already paid for. A cancellation with no date reads as "it stops
        // now", which is the one thing it does not do.
        body: l10n::settings_subscription_cancel_body(&paid_through(subscription)),
        go_ahead: l10n::settings_subscription_cancel(),
        // **Never "Cancel" for the way out.** On this question that word is the thing being asked
        // about, so the answer that does nothing has to say what it keeps.
        keep: l10n::settings_subscription_cancel_keep(),
        destructive: true,
    }
}

/// What moving between the two periods asks.
///
/// ⚠️ **No price here, deliberately.** What this subscriber is charged is not today's list price,
/// because a price change never reaches somebody who already subscribed, and quoting the list
/// price would tell a long-standing subscriber a number they will not be charged. The amount comes
/// back from the switch itself and is said afterwards.
fn switch_question(subscription: &AllodiaSubscription) -> Question {
    Question {
        title: l10n::settings_subscription_switch_title(),
        body: l10n::settings_subscription_switch_body(&paid_through(subscription)),
        go_ahead: l10n::action_update(),
        keep: l10n::action_cancel(),
        destructive: false,
    }
}

/// A button that asks first, and only sends the write when the answer is yes.
fn asking_button(
    sender: &relm4::Sender<AppInput>,
    window: &gtk::Window,
    label: &str,
    question: Question,
    write: SubscriptionWrite,
) -> gtk::Button {
    let button = button(label);
    let sender = sender.clone();
    let window = window.clone();
    button.connect_clicked(move |_| {
        let dialog = adw::AlertDialog::new(Some(question.title), Some(&question.body));
        dialog.add_response(RESPONSE_KEEP, question.keep);
        dialog.add_response(RESPONSE_GO_AHEAD, question.go_ahead);
        if question.destructive {
            dialog.set_response_appearance(RESPONSE_GO_AHEAD, adw::ResponseAppearance::Destructive);
        }
        // The answer that changes nothing is the default and the one Escape gives: a question
        // about somebody's money is not one to answer by pressing Return at the wrong moment.
        dialog.set_default_response(Some(RESPONSE_KEEP));
        dialog.set_close_response(RESPONSE_KEEP);
        let sender = sender.clone();
        let asked = dialog.choose_future(Some(&window));
        gtk::glib::MainContext::default().spawn_local(async move {
            if asked.await.as_str() == RESPONSE_GO_AHEAD {
                sender.emit(AppInput::AllodiaSubscription(SubscriptionInput::Write(
                    write,
                )));
            }
        });
    });
    button
}

/// The day both confirmations talk about: what has already been paid for.
///
/// The wire when this build cannot read it, which is the same fallback the section's own dates
/// take: a sentence with the wire in it is still true, and one with no date at all is not.
fn paid_through(subscription: &AllodiaSubscription) -> String {
    facts::paid_through(subscription)
        .and_then(|raw| timestamps::account_date(raw, l10n::active_locale()))
        .unwrap_or_default()
}

/// A read is running. No button of its own: it started itself, and there is nothing to press.
fn checking_row() -> adw::ActionRow {
    let row = text_row(l10n::settings_subscription_checking());
    let spinner = gtk::Spinner::new();
    spinner.start();
    row.add_suffix(&spinner);
    row
}

/// The one thing that fixes a sign-in older than the permission the read needs.
fn reauth_row(sender: &relm4::Sender<AppInput>) -> adw::ActionRow {
    let row = text_row(l10n::settings_subscription_reauth());
    let again = button(l10n::settings_allodia_reauth_action());
    let sender = sender.clone();
    again.connect_clicked(move |_| sender.emit(AppInput::StartAllodiaSignIn));
    row.add_suffix(&again);
    row
}

fn text_row(text: &str) -> adw::ActionRow {
    let row = plain_text_row();
    row.set_title(text);
    row
}

fn button(label: &str) -> gtk::Button {
    let button = gtk::Button::with_label(label);
    button.set_valign(gtk::Align::Center);
    button
}

#[cfg(test)]
#[path = "subscription_tests.rs"]
pub(crate) mod tests;
