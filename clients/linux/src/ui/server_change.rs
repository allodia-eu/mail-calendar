//! A calendar or address book paused because looking for its server again found one on another
//! site (`docs/account-autodetect.md` rule 14): the dialog that asks the person, and the allow it
//! sends. "Not now" needs no call: the use stays paused, and its page offers the question again.

use adw::prelude::*;
use mailcal_bindings::{ServerChange, ServerChangeAnswer};

use super::{AppInput, AppModel, account_settings::AccountsInput, settings::notice::Notice};
use crate::l10n;

const NOT_NOW: &str = "not-now";
const ALLOW: &str = "allow";

/// What the dialog is about: the server change, and the address of the account it moves.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Review {
    pub(super) address: String,
    pub(super) change: ServerChange,
}

/// Where the dialog was opened, which decides where a failed allow is said.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ReviewFrom {
    /// The account's page in Settings.
    Settings,
    /// The line above the calendar or the contacts.
    Outage,
}

/// Which uses moved. Both, or the one that did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Moved {
    Calendar,
    Contacts,
    Both,
}

const fn moved(change: &ServerChange) -> Moved {
    match (change.calendar, change.contacts) {
        (true, true) => Moved::Both,
        (false, true) => Moved::Contacts,
        _ => Moved::Calendar,
    }
}

fn title(change: &ServerChange) -> &'static str {
    match moved(change) {
        Moved::Calendar => l10n::server_changed_title_calendar(),
        Moved::Contacts => l10n::server_changed_title_contacts(),
        Moved::Both => l10n::server_changed_title_both(),
    }
}

fn body(change: &ServerChange, address: &str) -> String {
    match moved(change) {
        Moved::Calendar => l10n::server_changed_body_calendar(address),
        Moved::Contacts => l10n::server_changed_body_contacts(address),
        Moved::Both => l10n::server_changed_body_both(address),
    }
}

fn unsure(change: &ServerChange) -> &'static str {
    match moved(change) {
        Moved::Calendar => l10n::server_changed_unsure_calendar(),
        Moved::Contacts => l10n::server_changed_unsure_contacts(),
        Moved::Both => l10n::server_changed_unsure_both(),
    }
}

/// `line` with `site` in bold, as Pango markup. The sentence is escaped whole around a mark that
/// stands for the site, so a translation's own `&` or `<` is text, and only the site is bold.
fn bold_site(line: impl Fn(&str) -> String, site: &str) -> String {
    const MARK: &str = "\u{e000}";
    let site = format!("<b>{}</b>", gtk::glib::markup_escape_text(site));
    gtk::glib::markup_escape_text(&line(MARK)).replace(MARK, &site)
}

/// The dialog's body, as Pango markup: which account, the two sites, and what to weigh.
fn body_markup(review: &Review) -> String {
    let change = &review.change;
    let escape = |text: &str| gtk::glib::markup_escape_text(text).to_string();
    format!(
        "{}\n\n{}\n{}\n\n{}\n\n{}",
        escape(&body(change, &review.address)),
        bold_site(l10n::server_changed_before, &change.previous_site),
        bold_site(l10n::server_changed_now, &change.proposed_site),
        escape(l10n::server_changed_advice()),
        escape(unsure(change)),
    )
}

/// The question. "Not now" is the default and what Escape gives, so neither Return nor Escape
/// ever allows a server the person has not looked at.
fn dialog(review: &Review) -> adw::AlertDialog {
    let dialog = adw::AlertDialog::new(Some(title(&review.change)), None);
    dialog.set_body_use_markup(true);
    dialog.set_body(&body_markup(review));
    dialog.add_response(NOT_NOW, l10n::server_changed_not_now());
    dialog.add_response(ALLOW, l10n::server_changed_allow());
    dialog.set_default_response(Some(NOT_NOW));
    dialog.set_close_response(NOT_NOW);
    dialog
}

/// Asks over `parent` whether to allow `account`'s new server, and sends the allow when the
/// answer is yes. An alert dialog closes on every response, so the connect that follows is said
/// where it was asked from rather than in the dialog.
pub(super) fn ask(
    parent: &impl IsA<gtk::Widget>,
    account: &str,
    review: &Review,
    from: ReviewFrom,
    sender: &relm4::Sender<AppInput>,
) {
    let asked = dialog(review).choose_future(Some(parent));
    let (sender, account, review) = (sender.clone(), account.to_owned(), review.clone());
    gtk::glib::MainContext::default().spawn_local(async move {
        if asked.await.as_str() == ALLOW {
            sender.emit(AppInput::Accounts(AccountsInput::AllowServerChange {
                account,
                review,
                from,
            }));
        }
    });
}

/// What a failed allow says: nothing changed, and the use stays paused.
pub(super) fn failed() -> Notice {
    Notice::Error {
        title: l10n::settings_account_change_failed_title().to_owned(),
        detail: l10n::server_changed_failed().to_owned(),
    }
}

/// What follows an allow. A failure asked from the calendar or the contacts opens the account's
/// page, where the question is offered again, and says it there. A question a lookup replaced
/// while it was on screen moved nothing, and is asked again as it stands now.
fn allowed(
    answer: Option<ServerChangeAnswer>,
    account: String,
    review: Review,
    from: ReviewFrom,
) -> AccountsInput {
    match (answer, from) {
        (Some(ServerChangeAnswer::Moved), _) => AccountsInput::Changed(None),
        (Some(ServerChangeAnswer::Replaced { change }), _) => AccountsInput::ServerChangeReplaced {
            account,
            review: Review { change, ..review },
            from,
        },
        (None, ReviewFrom::Settings) => AccountsInput::Refused(failed()),
        (None, ReviewFrom::Outage) => AccountsInput::ServerChangeFailed(account),
    }
}

impl AppModel {
    /// Allows the new server if it is still the one `review` showed. The core connects to it
    /// before anything is stored, so it runs off the main thread.
    pub(super) fn allow_server_change(
        &mut self,
        account: String,
        review: Review,
        from: ReviewFrom,
        sender: relm4::Sender<AppInput>,
    ) {
        let Some(app) = self.app.clone() else {
            return;
        };
        if from == ReviewFrom::Settings {
            self.settings.say(Notice::Toast(
                l10n::settings_account_servers_trying().to_owned(),
            ));
        }
        std::thread::spawn(move || {
            let answer = app
                .accept_server_change(account.clone(), review.change.clone())
                .ok();
            sender.emit(AppInput::Accounts(allowed(answer, account, review, from)));
        });
    }

    /// Asks `review` over the window the person answered the question it replaced in.
    pub(super) fn ask_server_change_again(
        account: &str,
        review: &Review,
        from: ReviewFrom,
        sender: &relm4::Sender<AppInput>,
    ) {
        if let Some(window) = relm4::main_adw_application().active_window() {
            ask(&window, account, review, from, sender);
        }
    }
}

#[cfg(test)]
#[path = "server_change_tests.rs"]
mod tests;
