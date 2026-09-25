//! The Outbox: every account's unsent messages, in one list, and the three things a row offers.
//!
//! The rules are in `docs/sending.md`; the pane row that opens this list is rule 18 of
//! `docs/folder-pane.md`. What this file owns is the GTK half plus the one decision a list row
//! makes: what a queued send says about itself, and whether it may be acted on at all.
//!
//! The projection is deliberately widget-free and testable ([`state_label`], [`is_actionable`],
//! [`recipients_line`]), because the failure it guards against is silent on screen: a state
//! mapped to the wrong word tells someone their message is waiting when it is already on its
//! way, and offering "Send now" on a message that may have been delivered is how it arrives
//! twice.

use adw::prelude::*;
use gtk::accessible::Property as AccessibleProperty;
use mailcal_bindings::{Intent, MailboxListSnapshot, OutboxIntent, QueuedRow, QueuedState};

use super::{AppInput, AppModel, PrimaryView, mailbox, row_action};
use crate::{l10n, ui::icons};

/// One of the three things a queued send can be asked to do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum QueuedAction {
    SendNow,
    Cancel,
    Edit,
}

/// A queued send, named the only way it can be named.
///
/// The account **and** the op together: an op id is unique only within its own account's queue,
/// and this list is the one place holding every account's at once, so an id alone would be
/// resolved against whichever account happened to be selected.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct QueuedTarget {
    pub(crate) account: String,
    pub(crate) op: u64,
}

/// What a queued send's state is called on screen.
pub(crate) fn state_label(state: QueuedState) -> &'static str {
    match state {
        QueuedState::Sending => l10n::outbox_sending(),
        QueuedState::Unconfirmed => l10n::outbox_unconfirmed(),
        QueuedState::Waiting => l10n::outbox_waiting(),
    }
}

/// Whether a row offers its three actions at all.
///
/// A message in flight is on its way and cannot be called back; one whose delivery could not be
/// confirmed may already be in front of its recipients, and offering to send that again is how
/// it arrives twice (`docs/sending.md`). Both show their state and offer nothing.
pub(crate) fn is_actionable(state: QueuedState) -> bool {
    state == QueuedState::Waiting
}

/// Who the message is for, falling back to the account it would go out from.
///
/// An empty line reads as a rendering fault rather than as a fact, and `to` genuinely comes back
/// empty for a message addressed by Bcc alone; the account is then the only thing known about
/// where it is going.
pub(crate) fn recipients_line(row: &QueuedRow, account_email: &str) -> String {
    if row.to.is_empty() {
        return account_email.to_owned();
    }
    row.to.clone()
}

/// The subject, or the same words the message list uses for mail that has none.
pub(crate) fn subject_line(row: &QueuedRow) -> String {
    if row.subject.is_empty() {
        return l10n::mail_no_subject().to_owned();
    }
    row.subject.clone()
}

/// The address a queued row's account is known by, or the raw id if the account has since gone.
pub(crate) fn account_email(snapshot: &MailboxListSnapshot, account: &str) -> String {
    snapshot
        .accounts
        .iter()
        .find(|row| row.id == account)
        .map_or_else(|| account.to_owned(), |row| row.email.clone())
}

/// The whole row as one sentence, for the tooltip a truncated row needs.
///
/// The same four things the row draws, in the order it draws them, so what a tooltip says and
/// what a screen reader reads off the row's own parts cannot drift apart.
pub(crate) fn row_a11y(
    recipients: &str,
    subject: &str,
    state: QueuedState,
    account: &str,
) -> String {
    format!("{recipients}. {subject}. {}. {account}", state_label(state))
}

/// Draws the queued sends, oldest first: the order they will go out in.
pub(super) fn render(
    list: &gtk::ListBox,
    snapshot: &MailboxListSnapshot,
    sender: &relm4::Sender<AppInput>,
) {
    mailbox::install_styles();
    mailbox::clear(list);
    if snapshot.outbox.is_empty() {
        list.append(&empty_row());
        return;
    }
    for row in &snapshot.outbox {
        list.append(&queued_row(
            row,
            &account_email(snapshot, &row.account),
            sender,
        ));
    }
}

/// What the list says once the last queued message has gone or been withdrawn.
///
/// Reachable without navigating: cancelling the last row empties the list the user is looking
/// at, and an empty list with no words in it reads as a failure to load.
fn empty_row() -> adw::ActionRow {
    let row = mailbox::plain_text_row();
    row.set_title(l10n::outbox_empty());
    row.set_title_lines(2);
    row.set_selectable(false);
    row.set_activatable(false);
    row
}

/// One unsent message: who it is for, what it says, where the send has got to, and which account
/// it goes out from.
///
/// The account is on **every** row, not just an ambiguous one (`docs/folder-pane.md`, rule 18):
/// this list is the one place in the app holding every account's mail at once, and which identity
/// a message is waiting on is usually the whole of why it is waiting.
fn queued_row(row: &QueuedRow, account: &str, sender: &relm4::Sender<AppInput>) -> adw::ActionRow {
    let recipients = recipients_line(row, account);
    let subject = subject_line(row);
    // Recipients and subjects are the user's own text: a markup-shaped subject must render as
    // itself, and a bare ampersand must not be read as an entity.
    let widget = mailbox::plain_text_row();
    widget.set_title(&subject);
    widget.set_subtitle(&recipients);
    widget.set_title_lines(1);
    widget.set_subtitle_lines(1);
    widget.add_prefix(&gtk::Image::from_icon_name(state_icon(row.state)));
    widget.add_suffix(&state_label_widget(row.state, account));
    // A subject and an address are each as long as they are, and this list has four things to
    // fit on one line; the row that gets truncated is the one the user is looking for.
    widget.set_tooltip_text(Some(&row_a11y(&recipients, &subject, row.state, account)));
    if is_actionable(row.state) {
        widget.add_suffix(&menu_button(
            &QueuedTarget {
                account: row.account.clone(),
                op: row.op,
            },
            sender,
        ));
    }
    widget
}

/// The state's own glyph. Only the unconfirmed one warns: it is the single state a person may
/// need to go and check on another device.
fn state_icon(state: QueuedState) -> &'static str {
    match state {
        QueuedState::Sending => icons::SENDING,
        QueuedState::Unconfirmed => icons::WARNING,
        QueuedState::Waiting => icons::WAITING,
    }
}

/// The state, and under it the account this message goes out from.
fn state_label_widget(state: QueuedState, account: &str) -> gtk::Box {
    // No `Presentation` role on the column: GTK prunes a presentational container's children
    // out of the accessibility tree with it, and these two labels are the row's state and the
    // account it goes out from, which rule 18 and `docs/sending.md` both require it to say. An
    // unlabelled box is not announced on its own anyway.
    let column = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .valign(gtk::Align::Center)
        .build();
    let state_label_text = gtk::Label::new(Some(state_label(state)));
    state_label_text.add_css_class("dim-label");
    state_label_text.add_css_class("caption");
    state_label_text.set_halign(gtk::Align::End);
    let account_label = gtk::Label::new(Some(account));
    account_label.add_css_class("dim-label");
    account_label.add_css_class("caption");
    account_label.set_halign(gtk::Align::End);
    account_label.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
    account_label.set_max_width_chars(24);
    column.append(&state_label_text);
    column.append(&account_label);
    column
}

/// Send now, Edit and Cancel, on a row that may still be acted on.
///
/// Absent rather than disabled on the other two states, which is the Apple client's answer to
/// the same rule: a menu button that opens onto nothing is a worse offer than no button.
fn menu_button(target: &QueuedTarget, sender: &relm4::Sender<AppInput>) -> gtk::Box {
    let menu = gtk::Box::new(gtk::Orientation::Vertical, 0);
    menu.set_margin_top(6);
    menu.set_margin_bottom(6);
    menu.set_margin_start(6);
    menu.set_margin_end(6);
    for (label, action) in [
        (l10n::action_send_now(), QueuedAction::SendNow),
        (l10n::action_edit_queued(), QueuedAction::Edit),
        (l10n::action_cancel_send(), QueuedAction::Cancel),
    ] {
        let item = gtk::Button::with_label(label);
        item.add_css_class("flat");
        if action == QueuedAction::Cancel {
            item.add_css_class("destructive-action");
        }
        let input = sender.clone();
        let target = target.clone();
        item.connect_clicked(move |button| {
            if let Some(popover) = button
                .ancestor(gtk::Popover::static_type())
                .and_downcast::<gtk::Popover>()
            {
                popover.popdown();
            }
            input.emit(AppInput::QueuedSendAction {
                target: target.clone(),
                action,
            });
        });
        menu.append(&item);
    }
    let popover = gtk::Popover::new();
    popover.set_child(Some(&menu));
    let button = gtk::Button::from_icon_name(icons::MORE);
    button.set_tooltip_text(Some(l10n::a11y_more_actions()));
    button.update_property(&[AccessibleProperty::Label(l10n::a11y_more_actions())]);
    button.add_css_class("flat");
    button.set_valign(gtk::Align::Center);
    popover.set_parent(&button);
    let menu = popover.clone();
    button.connect_clicked(move |_| menu.popup());
    button.connect_destroy(move |_| {
        popover.popdown();
        popover.unparent();
    });
    let container = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    container.append(&button);
    container
}

/// The pane's Outbox row: one row above the account trees, **only while something is in it**.
///
/// Not inside a tree, because it is not a folder on anybody's server, and not per account,
/// because "did that go?" is not a question about a particular mailbox (`docs/folder-pane.md`,
/// rule 18). Its badge is the one number in this pane counting something other than unread mail,
/// so it gets a sentence of its own: those are messages nobody has read, these are messages
/// nobody has received.
pub(super) fn pane_row(queued: usize, sender: &relm4::Sender<AppInput>) -> adw::ActionRow {
    let row = mailbox::plain_text_row();
    row.set_title(l10n::folder_outbox());
    row.set_title_lines(1);
    row.set_activatable(true);
    row.add_prefix(&gtk::Image::from_icon_name(icons::OUTBOX));
    let input = sender.clone();
    row_action::action_row(&row, move || input.emit(AppInput::ShowOutbox));
    let count = i64::try_from(queued).unwrap_or(i64::MAX);
    let label = mailbox::badge(&count.to_string());
    let spoken = l10n::a11y_outbox_count(count);
    label.set_tooltip_text(Some(&spoken));
    label.update_property(&[AccessibleProperty::Label(&spoken)]);
    row.add_suffix(&label);
    row
}

impl AppModel {
    /// Opens the Outbox: every account's unsent messages, in one list.
    pub(super) fn show_outbox(&mut self) {
        // A pane row is a mail destination, so it takes the primary view back from the calendar.
        self.primary = PrimaryView::Mail;
        self.dispatch(Intent::Outbox {
            intent: OutboxIntent::Show,
        });
    }

    /// Sends now, withdraws, or reopens a queued message.
    ///
    /// `Edit` only asks: the core withdraws the message first and then offers it back through
    /// `Surface::ComposeRequest`, which is where this client opens its composer.
    pub(super) fn queued_send_action(&self, target: &QueuedTarget, action: QueuedAction) {
        let account = target.account.clone();
        let op = target.op;
        self.dispatch(Intent::Outbox {
            intent: match action {
                QueuedAction::SendNow => OutboxIntent::SendNow { account, op },
                QueuedAction::Cancel => OutboxIntent::Cancel { account, op },
                QueuedAction::Edit => OutboxIntent::Edit { account, op },
            },
        });
    }
}

#[cfg(test)]
#[path = "outbox_tests.rs"]
pub(crate) mod tests;
