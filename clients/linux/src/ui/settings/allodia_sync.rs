//! Settings → Accounts: what the person's other devices have to say, above their own mail
//! accounts.
//!
//! Its Apple, Android and Windows twins draw the same three things in the same order; keep the
//! states and the wording in step.
//!
//! It sits in Accounts rather than in the Allodia-account category because what it is about is mail
//! accounts: one arriving is an account to set up, and that is where somebody looks for it.

use adw::prelude::*;
use mailcal_bindings::{
    AccountPageIntent, AllodiaAccountSyncMode, AllodiaDevicesQuestion, AllodiaElsewhere,
    AllodiaSyncFailure,
};

use super::{PageContext, account_sync_mode, group};
use crate::{
    l10n,
    ui::{AppInput, account_settings::AccountsInput, controls, mailbox::plain_text_row},
};

/// The group, or `None` when there is nothing to say: before the first pass has answered, which
/// must not look like a pass that found nothing (a heading over an empty list claims the other
/// devices hold no accounts, which this device does not know yet), and once one found nothing.
pub(super) fn allodia_sync(ctx: &PageContext) -> Option<adw::PreferencesGroup> {
    let devices = ctx.devices.as_ref().filter(|devices| !devices.nothing_found)?;
    let section = group(
        l10n::settings_allodia_sync_heading(),
        l10n::settings_allodia_sync_description(),
    );
    if devices.checking {
        section.add(&checking_row());
    }
    for offer in &devices.offers {
        section.add(&offer_row(&ctx.sender, offer));
    }
    // The only answer this device can act on is keeping what it has. Applying the other side's
    // settings needs a path for editing a connected account's server details, which does not
    // exist yet.
    for question in &devices.questions {
        section.add(&question_row(&ctx.sender, question));
        if let Some(failed) = question.failed {
            section.add(&account_sync_mode::failed_row(failed));
        }
    }
    if let Some(failure) = devices.failure {
        add_failure(&section, &ctx.sender, failure);
    }
    Some(section)
}

/// What a failed pass is allowed to put on screen.
///
/// The core's typed answer decides, never the failure's text. A grant that predates a permission
/// and one the service revoked are different sentences with different remedies, and everything else
/// says nothing about the sign-in at all; so it gets one plain line and the detail stays in the
/// diagnostic log. There is no longer a path from an error's text to a row, which is what put
/// `invalid_scope: unable to issue scope mailcal:accounts:read` in front of somebody.
fn add_failure(
    section: &adw::PreferencesGroup,
    sender: &relm4::Sender<AppInput>,
    failure: AllodiaSyncFailure,
) {
    match failure {
        // An offer, not an error: they are signed in and one feature is asleep. The remedy is the
        // ordinary sign-in, which asks for the full current scope set every time.
        AllodiaSyncFailure::NeedsReauth => section.add(&reauth_row(
            sender,
            l10n::settings_allodia_reauth(),
            l10n::settings_allodia_reauth_hint(),
            l10n::settings_allodia_reauth_action(),
        )),
        AllodiaSyncFailure::SignedOut => section.add(&reauth_row(
            sender,
            l10n::settings_allodia_signed_out_elsewhere(),
            l10n::settings_allodia_signed_out_elsewhere_hint(),
            l10n::settings_allodia_sign_in(),
        )),
        AllodiaSyncFailure::Unavailable => {
            section.add(&failure_row(l10n::settings_allodia_sync_unavailable()));
        }
    }
}

/// A row that says what is wrong and carries the one button that fixes it.
///
/// `use_markup(false)` on both, like every row here: a title or subtitle parsed as Pango renders
/// blank on a bare ampersand, and these are localised sentences (`AGENTS.md` → Client conventions).
/// Set with the setters rather than the builder, so the flag is off before either text lands.
fn reauth_row(
    sender: &relm4::Sender<AppInput>,
    title: &str,
    subtitle: &str,
    action: &str,
) -> adw::ActionRow {
    let row = plain_text_row();
    row.set_title(title);
    row.set_subtitle(subtitle);
    let button = gtk::Button::with_label(action);
    button.set_valign(gtk::Align::Center);
    let sender = sender.clone();
    button.connect_clicked(move |_| sender.emit(AppInput::StartAllodiaSignIn));
    row.add_suffix(&button);
    row
}

/// A pass is running. No button of its own: it started itself, and there is nothing to press.
fn checking_row() -> adw::ActionRow {
    let row = plain_text_row();
    row.set_title(l10n::settings_allodia_sync_checking());
    let spinner = gtk::Spinner::new();
    spinner.start();
    row.add_suffix(&spinner);
    row
}

/// An account from another device. The **address** is the title, because that is what a person
/// recognises; the rest of the record is what decides the route the button opens.
fn offer_row(
    sender: &relm4::Sender<AppInput>,
    offer: &mailcal_bindings::AllodiaAccountOffer,
) -> adw::ActionRow {
    let row = plain_text_row();
    row.set_title(&offer.email);
    let set_up = gtk::Button::with_label(l10n::settings_allodia_sync_set_up());
    set_up.add_css_class("suggested-action");
    set_up.set_valign(gtk::Align::Center);
    let sender = sender.clone();
    let offer = offer.clone();
    set_up.connect_clicked(move |_| {
        sender.emit(AppInput::SetUpOfferedAccount(Box::new(offer.clone())));
    });
    row.add_suffix(&set_up);
    row
}

/// An account that moved or went somewhere else, and the one answer this device can act on:
/// keeping what it has, which pauses the account's sharing, as a change on its page.
fn question_row(
    sender: &relm4::Sender<AppInput>,
    question: &AllodiaDevicesQuestion,
) -> adw::ActionRow {
    let row = plain_text_row();
    row.set_title(&match question.elsewhere {
        AllodiaElsewhere::Changed => l10n::settings_allodia_changed_elsewhere(&question.email),
        AllodiaElsewhere::Removed => l10n::settings_allodia_removed_elsewhere(&question.email),
    });
    let sender = sender.clone();
    let account = question.account_id.clone();
    row.add_suffix(&controls::button(
        l10n::settings_allodia_keep_local(),
        question.keep,
        false,
        move || {
            sender.emit(AppInput::Accounts(AccountsInput::Change(
                AccountPageIntent::SetSharing {
                    account: account.clone(),
                    mode: AllodiaAccountSyncMode::Paused,
                },
            )));
        },
    ));
    row
}

/// Why the last pass did not work. Its own row rather than the group's description, which says
/// what this section *is* and stays true whatever happened.
fn failure_row(failure: &str) -> adw::ActionRow {
    let row = plain_text_row();
    row.set_title(failure);
    row.add_css_class("error");
    row
}

#[cfg(test)]
#[path = "allodia_sync_tests.rs"]
pub(crate) mod tests;
