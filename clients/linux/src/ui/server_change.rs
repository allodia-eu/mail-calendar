//! A calendar or address book paused because looking for its server again found one on another
//! site (`docs/account-autodetect.md` rule 14): the question, drawn from the core's
//! `AccountServerChange` whether the account's page, the calendar or the contacts list opened it.
//! Allow is a change on the account's page; the question stays open until the account waits on
//! none, and says a failure itself. "Not now" needs no call: the use stays paused, and its page
//! offers the question again.

use adw::prelude::*;
use mailcal_bindings::{AccountPageIntent, AccountServerChange, ControlState, ServerChange};

use super::{AppInput, AppModel, account_settings::AccountsInput, controls};
use crate::l10n;

/// Numbers each question opened, so the close of one replaced is not taken for its successor's.
static OPENED: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// The question on screen: whose, the dialog, and the record it was drawn from last.
pub(crate) struct Question {
    serial: u64,
    account: String,
    dialog: adw::Dialog,
    drawn: AccountServerChange,
    sender: relm4::Sender<AppInput>,
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

/// The question's body, as Pango markup: which account, the two sites, and what to weigh.
fn body_markup(change: &ServerChange, address: &str) -> String {
    let escape = |text: &str| gtk::glib::markup_escape_text(text).to_string();
    format!(
        "{}\n\n{}\n{}\n\n{}\n\n{}",
        escape(&body(change, address)),
        bold_site(l10n::server_changed_before, &change.previous_site),
        bold_site(l10n::server_changed_now, &change.proposed_site),
        escape(l10n::server_changed_advice()),
        escape(unsure(change)),
    )
}

/// Draws `question` into `dialog`, and returns what it drew. Neither button
/// is the suggested one, and "Not now" is the default: Return never allows a server the person has
/// not looked at. While Allow runs, "Not now" and Escape wait for it too, since closing would not
/// stop it.
fn content(
    dialog: &adw::Dialog,
    question: &AccountServerChange,
    allow: impl Fn() + 'static,
) -> gtk::Box {
    let change = &question.change;
    let content = gtk::Box::new(gtk::Orientation::Vertical, 12);
    content.set_margin_top(24);
    content.set_margin_bottom(24);
    content.set_margin_start(24);
    content.set_margin_end(24);
    let heading = gtk::Label::new(Some(title(change)));
    heading.add_css_class("title-2");
    heading.set_wrap(true);
    heading.set_justify(gtk::Justification::Center);
    content.append(&heading);
    let text = gtk::Label::new(None);
    text.set_markup(&body_markup(
        change,
        &super::settings::account_names::linked_name(&question.account),
    ));
    text.set_wrap(true);
    text.set_xalign(0.0);
    content.append(&text);
    if question.failed {
        let failed = gtk::Label::new(Some(l10n::server_changed_failed()));
        failed.add_css_class("error");
        failed.set_wrap(true);
        failed.set_xalign(0.0);
        content.append(&failed);
    }
    let buttons = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    buttons.set_homogeneous(true);
    buttons.set_margin_top(12);
    let closing = dialog.clone();
    let not_now = controls::button(
        l10n::server_changed_not_now(),
        ControlState {
            enabled: !question.allow.running,
            running: false,
        },
        false,
        move || {
            closing.close();
        },
    );
    buttons.append(&not_now);
    buttons.append(&controls::button(
        l10n::server_changed_allow(),
        question.allow,
        false,
        allow,
    ));
    content.append(&buttons);
    // In the dialog before it takes focus, which only a widget the dialog holds can.
    dialog.set_child(Some(&content));
    dialog.set_default_widget(Some(&not_now));
    dialog.set_focus(Some(&not_now));
    // Drawn again after Allow has run, the question's controls are new, and the one that held
    // focus went with the old ones: nothing would take Escape. The default answer takes focus once
    // the dialog has settled on its new child; asked for any sooner, the dialog drops it.
    let focus = not_now.downgrade();
    gtk::glib::idle_add_local_once(move || {
        if let Some(button) = focus.upgrade().filter(gtk::prelude::WidgetExt::is_mapped) {
            button.grab_focus();
        }
    });
    dialog.set_can_close(!question.allow.running);
    dialog.set_title(title(change));
    content
}

impl Question {
    /// Draws `drawn` into the dialog, with Allow sending the change it shows.
    fn draw(&self) {
        let (sender, account, shown) = (
            self.sender.clone(),
            self.account.clone(),
            self.drawn.change.clone(),
        );
        content(&self.dialog, &self.drawn, move || {
            sender.emit(AppInput::Accounts(AccountsInput::Change(
                AccountPageIntent::AllowServerChange {
                    account: account.clone(),
                    shown: shown.clone(),
                },
            )));
        });
    }
}

impl AppModel {
    /// Opens the question `account` is paused for, over the window the person is in: Settings
    /// or the main window. One settled since the person chose Review opens the account's page
    /// instead, which says where its uses stand now.
    pub(super) fn review_server_change(
        &mut self,
        account: String,
        sender: &relm4::Sender<AppInput>,
    ) {
        let Some(app) = self.app.clone() else {
            return;
        };
        let Some(drawn) = app.account_server_change(account.clone()) else {
            self.settings.open_account(account);
            return;
        };
        if let Some(open) = self.server_change.take() {
            open.dialog.force_close();
        }
        let dialog = adw::Dialog::new();
        dialog.set_content_width(440);
        let serial = OPENED.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let closed = sender.clone();
        dialog.connect_closed(move |_| {
            closed.emit(AppInput::Accounts(AccountsInput::ServerChangeClosed(
                serial,
            )));
        });
        let question = Question {
            serial,
            account,
            dialog,
            drawn,
            sender: sender.clone(),
        };
        question.draw();
        let parent = relm4::main_adw_application().active_window();
        question.dialog.present(parent.as_ref());
        self.server_change = Some(question);
    }

    /// Draws the open question again from the core, after a signal that may have moved it: an
    /// allow starting or ending, or a lookup replacing it. One the account no longer waits on
    /// closes.
    pub(super) fn server_change_signalled(&mut self) {
        let (Some(app), Some(question)) = (self.app.clone(), self.server_change.as_mut()) else {
            return;
        };
        match app.account_server_change(question.account.clone()) {
            None => {
                if let Some(question) = self.server_change.take() {
                    question.dialog.force_close();
                }
            }
            Some(now) if now != question.drawn => {
                question.drawn = now;
                question.draw();
            }
            Some(_) => {}
        }
    }

    /// The question opened as `serial` closed, by "Not now", Escape or being settled.
    pub(super) fn server_change_closed(&mut self, serial: u64) {
        if self
            .server_change
            .as_ref()
            .is_some_and(|question| question.serial == serial)
        {
            self.server_change = None;
        }
    }
}

#[cfg(test)]
#[path = "server_change_tests.rs"]
pub(crate) mod tests;
