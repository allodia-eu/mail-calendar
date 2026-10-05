//! The Outbox: every account's unsent messages, in one list, and what a row offers.
//!
//! The rules are in `docs/sending.md`; the pane row that opens this list is rule 18 of
//! `docs/folder-pane.md`. What this file owns is the GTK half plus the one decision a list row
//! makes: what a queued send says about itself, and what it may be asked to do.
//!
//! The projection is deliberately widget-free and testable ([`state_label`], [`actions`],
//! [`recipients_line`]), because the failure it guards against is silent on screen: a state
//! mapped to the wrong word tells someone their message is waiting when it is already on its
//! way, and offering "Send now" on a message that may have been delivered is how it arrives
//! twice.

use adw::prelude::*;
use gtk::accessible::Property as AccessibleProperty;
use mailcal_bindings::{Intent, MailboxListSnapshot, OutboxIntent, QueuedRow, QueuedState};

use super::{AppInput, AppModel, PrimaryView, mailbox, row_action};
use crate::{l10n, ui::icons};

/// One thing a queued send can be asked to do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum QueuedAction {
    /// Hurry a waiting send.
    SendNow,
    /// Send one the server refused, again.
    SendAgain,
    Edit,
    /// Withdraw a waiting send.
    Cancel,
    /// Take a refused send out of the Outbox.
    Discard,
    /// Answer an unconfirmed send: it arrived.
    MarkSent,
    /// Answer an unconfirmed send: it did not arrive, so send it again.
    ConfirmNotSent,
}

impl QueuedAction {
    /// The words on the menu item. The two ways of sending again read the same, because to the
    /// user they are the same request; which intent each sends is what tells them apart.
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::SendNow => l10n::action_send_now(),
            Self::SendAgain | Self::ConfirmNotSent => l10n::action_send_again(),
            Self::Edit => l10n::action_edit_queued(),
            Self::Cancel => l10n::action_cancel_send(),
            Self::Discard => l10n::action_discard(),
            Self::MarkSent => l10n::action_mark_sent(),
        }
    }

    /// Whether the item takes the message away for good.
    fn is_destructive(self) -> bool {
        matches!(self, Self::Cancel | Self::Discard)
    }

    /// Whether the user is asked first. Only sending again a message that may already have been
    /// delivered: the one action here whose mistake reaches other people's inboxes.
    pub(crate) fn needs_confirmation(self) -> bool {
        self == Self::ConfirmNotSent
    }

    /// The intent the action sends. Sending again is `SendNow` for a refused message and
    /// `ConfirmNotSent` for an unconfirmed one: the core refuses the second on any other state,
    /// so a row that changed under the click cannot send a message twice. `staging` is where
    /// Edit has the message's files written for the composer.
    pub(crate) fn intent(self, account: String, op: u64, staging: String) -> OutboxIntent {
        match self {
            Self::SendNow | Self::SendAgain => OutboxIntent::SendNow { account, op },
            Self::Edit => OutboxIntent::Edit {
                account,
                op,
                staging_directory: staging,
            },
            Self::Cancel | Self::Discard => OutboxIntent::Cancel { account, op },
            Self::MarkSent => OutboxIntent::ConfirmSent { account, op },
            Self::ConfirmNotSent => OutboxIntent::ConfirmNotSent { account, op },
        }
    }
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
        QueuedState::NotSent => l10n::outbox_not_sent(),
    }
}

/// What a row offers, in menu order (`docs/sending.md`).
///
/// A message in flight is on its way and cannot be called back, so it offers nothing. One whose
/// delivery could not be confirmed may already be in front of its recipients: it offers only
/// the two answers, never Send now, Edit or Cancel, because offering to send it again without
/// the user saying it did not arrive is how it arrives twice. Edit is left out of a message a
/// composer cannot hold (`editable`).
pub(crate) fn actions(state: QueuedState, editable: bool) -> &'static [QueuedAction] {
    use QueuedAction::{Cancel, ConfirmNotSent, Discard, Edit, MarkSent, SendAgain, SendNow};
    match (state, editable) {
        (QueuedState::Waiting, true) => &[SendNow, Edit, Cancel],
        (QueuedState::Waiting, false) => &[SendNow, Cancel],
        (QueuedState::Sending, _) => &[],
        (QueuedState::Unconfirmed, _) => &[MarkSent, ConfirmNotSent],
        (QueuedState::NotSent, true) => &[SendAgain, Edit, Discard],
        (QueuedState::NotSent, false) => &[SendAgain, Discard],
    }
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
    let offered = actions(row.state, row.editable);
    if !offered.is_empty() {
        widget.add_suffix(&menu_button(
            &QueuedTarget {
                account: row.account.clone(),
                op: row.op,
            },
            offered,
            sender,
        ));
    }
    widget
}

/// The state's own glyph. The unconfirmed one warns, because a person may need to go and check
/// on another device; the refused one says it did not go.
fn state_icon(state: QueuedState) -> &'static str {
    match state {
        QueuedState::Sending => icons::SENDING,
        QueuedState::Unconfirmed => icons::WARNING,
        QueuedState::Waiting => icons::WAITING,
        QueuedState::NotSent => icons::NOT_SENT,
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

/// The row's menu, holding what its state offers.
///
/// Absent rather than disabled on a row that offers nothing, which is the Apple client's answer
/// to the same rule: a menu button that opens onto nothing is a worse offer than no button.
fn menu_button(
    target: &QueuedTarget,
    offered: &[QueuedAction],
    sender: &relm4::Sender<AppInput>,
) -> gtk::Box {
    let menu = gtk::Box::new(gtk::Orientation::Vertical, 0);
    menu.set_margin_top(6);
    menu.set_margin_bottom(6);
    menu.set_margin_start(6);
    menu.set_margin_end(6);
    for &action in offered {
        let item = gtk::Button::with_label(action.label());
        item.add_css_class("flat");
        if action.is_destructive() {
            item.add_css_class("destructive-action");
        }
        let input = sender.clone();
        let target = target.clone();
        item.connect_clicked(move |button| {
            // Read before the popover closes: a popped-down popover is no longer in the window.
            let window = button.root().and_downcast::<gtk::Window>();
            if let Some(popover) = button
                .ancestor(gtk::Popover::static_type())
                .and_downcast::<gtk::Popover>()
            {
                popover.popdown();
            }
            match window.filter(|_| action.needs_confirmation()) {
                Some(window) => confirm_send_again(&window, target.clone(), &input),
                None => input.emit(AppInput::QueuedSendAction {
                    target: target.clone(),
                    action,
                }),
            }
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

/// Asks before a message that may already have been delivered is sent again, and sends it only
/// on the user's yes.
fn confirm_send_again(
    parent: &gtk::Window,
    target: QueuedTarget,
    sender: &relm4::Sender<AppInput>,
) {
    let (window, _) = crate::ui::modal::new(parent, l10n::outbox_send_again_title(), 420, None);
    window.set_resizable(false);
    let content = gtk::Box::new(gtk::Orientation::Vertical, 18);
    content.set_margin_top(24);
    content.set_margin_bottom(24);
    content.set_margin_start(24);
    content.set_margin_end(24);
    let message = gtk::Label::new(Some(l10n::outbox_send_again_message()));
    message.set_wrap(true);
    message.set_xalign(0.0);
    content.append(&message);
    let buttons = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    buttons.set_halign(gtk::Align::End);
    let cancel = gtk::Button::with_label(l10n::action_cancel());
    let dialog = window.clone();
    cancel.connect_clicked(move |_| dialog.close());
    buttons.append(&cancel);
    let send = gtk::Button::with_label(l10n::action_send_again());
    send.add_css_class("destructive-action");
    let input = sender.clone();
    let dialog = window.clone();
    send.connect_clicked(move |_| {
        input.emit(AppInput::QueuedSendAction {
            target: target.clone(),
            action: QueuedAction::ConfirmNotSent,
        });
        dialog.close();
    });
    buttons.append(&send);
    content.append(&buttons);
    window.set_child(Some(&content));
    window.set_default_widget(Some(&cancel));
    window.present();
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

    /// Sends, withdraws, reopens or answers for a queued message.
    ///
    /// `Edit` only asks: the core withdraws the message first and then offers it back through
    /// `Surface::ComposeRequest`, which is where this client opens its composer.
    pub(super) fn queued_send_action(&self, target: &QueuedTarget, action: QueuedAction) {
        let staging = if action == QueuedAction::Edit {
            edit_staging_dir(target.op)
        } else {
            String::new()
        };
        self.dispatch(Intent::Outbox {
            intent: action.intent(target.account.clone(), target.op, staging),
        });
    }
}

/// Where an edited message's files are written for the composer: under the user's cache, owner
/// only, as for a resumed draft's (`docs/drafts.md`).
fn edit_staging_dir(op: u64) -> String {
    use std::os::unix::fs::PermissionsExt;
    let directory = gtk::glib::user_cache_dir()
        .join("mailcal")
        .join(format!("outbox-edit-{}-{op}", std::process::id()));
    if std::fs::create_dir_all(&directory).is_ok() {
        let _ = std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o700));
    }
    directory.to_string_lossy().into_owned()
}

#[cfg(test)]
#[path = "outbox_tests.rs"]
pub(crate) mod tests;
