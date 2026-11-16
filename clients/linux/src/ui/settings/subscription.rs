//! Settings → Allodia account → Subscription: what is being charged, by whom, until when, the way
//! to start one, and the three things somebody may do to the subscription Allodia bills directly.
//!
//! **What this file draws is what the core decided** (`MailcalApp::allodia_subscription_card`):
//! which writes exist, whether one runs, which store pages to offer and what a write came to. A
//! store's subscription is changed only at that store, so its button opens the store's page.
//!
//! ⚠️ **Linux sells through Allodia's own checkout**, a link opened in the browser: Flatpak has no
//! commerce. So the buy buttons price today's list from the card, in minor units this side
//! formats, in the order the core's `order_allodia_offers` gives.

use adw::prelude::*;
use mailcal_bindings::{
    AllodiaCheckoutOffer, AllodiaOffer, AllodiaPlan, AllodiaStore, AllodiaSubscriptionState,
    AllodiaSubscriptionView, ControlState,
};

use super::{PageContext, group_titled};
use crate::{
    l10n,
    ui::{
        AppInput,
        allodia_subscription::{SubscriptionInput, SubscriptionWrite, outcome_text},
        allodia_subscription_facts as facts, controls,
        mailbox::plain_text_row,
        timestamps,
    },
};

const RESPONSE_KEEP: &str = "keep";
const RESPONSE_GO_AHEAD: &str = "go-ahead";

/// The section, or `None` while the core draws none: nobody signed in, or a sign-in the service
/// refused, which the account card says and carries the remedy for.
pub(super) fn subscription(ctx: &PageContext) -> Option<adw::PreferencesGroup> {
    let card = ctx.app.allodia_subscription_card();
    let section = group_titled(l10n::settings_subscription_heading());
    match &card.state {
        AllodiaSubscriptionState::Hidden => return None,
        // Nothing read yet, so the page that draws it asks now; a read already out answers for
        // this one, and the core drops the second.
        AllodiaSubscriptionState::Checking => {
            ctx.sender
                .emit(AppInput::AllodiaSubscription(SubscriptionInput::Refresh));
            section.add(&checking_row());
        }
        // Deliberately quiet: a read that did not arrive costs a sentence, never access.
        AllodiaSubscriptionState::Unavailable => {
            section.add(&text_row(l10n::settings_subscription_unavailable()));
        }
        AllodiaSubscriptionState::NeedsReauth => section.add(&reauth_row(&ctx.sender)),
        AllodiaSubscriptionState::Loaded { view } => loaded(&section, ctx, view),
    }
    if let Some(outcome) = &card.outcome {
        section.add(&text_row(&outcome_text(outcome, l10n::active_locale())));
    }
    Some(section)
}

fn loaded(section: &adw::PreferencesGroup, ctx: &PageContext, view: &AllodiaSubscriptionView) {
    // A read beside the answer, after a return from the checkout browser: the answer stays drawn.
    if view.refreshing {
        let spinner = adw::Spinner::new();
        spinner.set_valign(gtk::Align::Center);
        section.set_header_suffix(Some(&spinner));
    }
    if view.entitled {
        section.add(&active_row(&ctx.sender, &ctx.window, view));
        // Not a lapse, and never drawn as one: the charge is being retried and access continues.
        if view.grace
            && let Some(biller) = &view.billed_by
        {
            section.add(&text_row(&l10n::settings_subscription_grace(
                &facts::biller_name(biller),
            )));
        }
        // Reported, never resolved: cancelling one without asking is a decision about somebody's
        // money, so each exit is offered and none is taken.
        if !view.duplicate_billing.is_empty() {
            let names: Vec<String> = view
                .duplicate_billing
                .iter()
                .map(facts::biller_name)
                .collect();
            let row = text_row(&l10n::settings_subscription_duplicate(&names.join(", ")));
            row.add_css_class("error");
            section.add(&row);
        }
    } else {
        let offers = view
            .checkout
            .as_ref()
            .map(|checkout| offers(ctx, checkout))
            .unwrap_or_default();
        let row = free_row(&ctx.sender, &offers);
        for manage in store_page_buttons(&ctx.sender, view) {
            row.add_suffix(&manage);
        }
        section.add(&row);
    }
}

/// What a paid account says: who is charging, until when, and everything that may be done about
/// it, in one row.
fn active_row(
    sender: &relm4::Sender<AppInput>,
    window: &gtk::Window,
    view: &AllodiaSubscriptionView,
) -> adw::ActionRow {
    let row = plain_text_row();
    if let Some(biller) = &view.billed_by {
        row.set_title(&l10n::settings_subscription_billed_by(&facts::biller_name(
            biller,
        )));
    }
    if let Some(end) = view
        .period_end
        .as_deref()
        .and_then(|raw| timestamps::account_date(raw, l10n::active_locale()))
    {
        row.set_subtitle(&if view.renews {
            l10n::settings_subscription_renews(&end)
        } else {
            l10n::settings_subscription_ends(&end)
        });
    }
    for manage in store_page_buttons(sender, view) {
        row.add_suffix(&manage);
    }
    for write in write_buttons(sender, window, view) {
        row.add_suffix(&write);
    }
    row
}

/// One button per store with something left to manage there, each opening that store's page: a
/// store's subscription is changed there and nowhere else. Drawn on the free half too, since on
/// hold and paused grant nothing and are put right only at the store.
fn store_page_buttons(
    sender: &relm4::Sender<AppInput>,
    view: &AllodiaSubscriptionView,
) -> Vec<gtk::Button> {
    view.store_pages
        .iter()
        .map(|page| {
            let manage = gtk::Button::with_label(&l10n::settings_subscription_manage(
                facts::store_name(page.store),
            ));
            manage.set_valign(gtk::Align::Center);
            let (url, sender) = (page.url.clone(), sender.clone());
            manage.connect_clicked(move |_| {
                sender.emit(AppInput::AllodiaSubscription(
                    SubscriptionInput::OpenStorePage(url.clone()),
                ));
            });
            manage
        })
        .collect()
}

/// What may be done to Allodia's own subscription, as the core decided it.
fn write_buttons(
    sender: &relm4::Sender<AppInput>,
    window: &gtk::Window,
    view: &AllodiaSubscriptionView,
) -> Vec<gtk::Button> {
    let mut buttons = Vec::new();
    if let Some(switch) = &view.switch {
        let label = if switch.to == AllodiaPlan::Yearly {
            l10n::settings_subscription_switch_yearly()
        } else {
            l10n::settings_subscription_switch_monthly()
        };
        buttons.push(asking_button(
            sender,
            window,
            label,
            switch.control,
            switch_question(view),
            SubscriptionWrite::Switch(switch.to),
        ));
    }
    if let Some(control) = view.resubscribe {
        buttons.push(write_button(
            sender,
            l10n::settings_subscription_resubscribe(),
            control,
            SubscriptionWrite::Resubscribe,
        ));
    }
    if let Some(control) = view.cancel {
        buttons.push(asking_button(
            sender,
            window,
            l10n::settings_subscription_cancel(),
            control,
            cancel_question(view),
            SubscriptionWrite::Cancel,
        ));
    }
    buttons
}

/// A button that sends `write` on the press.
fn write_button(
    sender: &relm4::Sender<AppInput>,
    label: &str,
    control: ControlState,
    write: SubscriptionWrite,
) -> gtk::Button {
    let sender = sender.clone();
    controls::button(label, control, false, move || {
        sender.emit(AppInput::AllodiaSubscription(SubscriptionInput::Write(
            write,
        )));
    })
}

/// What an account nobody is charging says, and the two periods it may be bought for.
fn free_row(
    sender: &relm4::Sender<AppInput>,
    offers: &[(AllodiaOffer, ControlState)],
) -> adw::ActionRow {
    let row = text_row(l10n::settings_subscription_free());
    if offers.is_empty() {
        return row;
    }
    // What renewing means, beside the buttons and not a click away: somebody agreeing to a
    // recurring charge is owed it.
    row.set_subtitle(l10n::settings_subscription_terms());
    for (offer, control) in offers {
        // **The period is on the button, not under it.** It is the thing being chosen.
        let label = if offer.plan == AllodiaPlan::Yearly {
            l10n::settings_subscription_buy_yearly(&offer.display_price)
        } else {
            l10n::settings_subscription_buy_monthly(&offer.display_price)
        };
        let buy = write_button(
            sender,
            &label,
            *control,
            SubscriptionWrite::Checkout(offer.plan),
        );
        buy.add_css_class("suggested-action");
        row.add_suffix(&buy);
    }
    row
}

/// The plans Allodia's checkout sells, priced from today's list, in the core's order.
fn offers(ctx: &PageContext, checkout: &AllodiaCheckoutOffer) -> Vec<(AllodiaOffer, ControlState)> {
    let locale = l10n::active_locale();
    let prices = &checkout.prices;
    let offer = |plan, minor| AllodiaOffer {
        plan,
        store: AllodiaStore::Allodia,
        display_price: facts::minor_units(minor, &prices.currency, locale),
    };
    ctx.app
        .order_allodia_offers(vec![
            offer(AllodiaPlan::Yearly, prices.yearly_in_cents),
            offer(AllodiaPlan::Monthly, prices.monthly_in_cents),
        ])
        .into_iter()
        .map(|offer| {
            let control = if offer.plan == AllodiaPlan::Yearly {
                checkout.yearly
            } else {
                checkout.monthly
            };
            (offer, control)
        })
        .collect()
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
fn cancel_question(view: &AllodiaSubscriptionView) -> Question {
    Question {
        title: l10n::settings_subscription_cancel_title(),
        // ⚠️ The date is the whole reassurance: somebody cancelling wants to know they are not
        // losing what they have already paid for. A cancellation with no date reads as "it stops
        // now", which is the one thing it does not do.
        body: l10n::settings_subscription_cancel_body(&paid_through(view)),
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
fn switch_question(view: &AllodiaSubscriptionView) -> Question {
    Question {
        title: l10n::settings_subscription_switch_title(),
        body: l10n::settings_subscription_switch_body(&paid_through(view)),
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
    control: ControlState,
    question: Question,
    write: SubscriptionWrite,
) -> gtk::Button {
    let sender = sender.clone();
    let window = window.clone();
    let destructive = question.destructive;
    controls::button(label, control, destructive, move || {
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
    })
}

/// The day both confirmations talk about: what has already been paid for.
///
/// The wire when this build cannot read it, which is the same fallback the section's own dates
/// take: a sentence with the wire in it is still true, and one with no date at all is not.
fn paid_through(view: &AllodiaSubscriptionView) -> String {
    view.paid_through
        .as_deref()
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
