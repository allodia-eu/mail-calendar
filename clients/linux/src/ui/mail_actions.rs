//! Mail-action projections for the Linux message list and reading pane.

use adw::prelude::*;
use gtk::accessible::Property as AccessibleProperty;
use mailcal_bindings::{BulkAction, FlatRow, FolderRole, Intent, MailboxListSnapshot, SnapshotRow};

use super::{AppInput, AppModel, mailbox, mailbox::ThreadKey, model};
use crate::l10n;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct MessageTarget {
    pub(crate) account: String,
    pub(crate) key: String,
}

/// What the "delete permanently?" confirmation is about to destroy.
///
/// Both paths ask, because both are irreversible; only the sentence differs, and the selection
/// arm carries the count so it can say how much is going (`docs/list-selection.md`, rule 6).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum DeleteTarget {
    /// One message, named from the row's own menu.
    Message(MessageTarget),
    /// Every selected row, and how many there are.
    Selection(usize),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ActionKind {
    MarkRead(bool),
    SetFlagged(bool),
    Archive,
    MoveToTrash,
    MarkAsSpam,
    MarkAsNotSpam,
    PermanentlyDelete,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct MailActionRequest {
    pub(crate) target: MessageTarget,
    pub(crate) action: ActionKind,
}

impl MailActionRequest {
    pub(crate) fn new(target: MessageTarget, action: ActionKind) -> Self {
        Self { target, action }
    }

    pub(crate) fn into_intent(self) -> Intent {
        intent_for(self.target, self.action)
    }
}

pub(super) fn actions_for(unread: bool, flagged: bool, in_junk_folder: bool) -> Vec<ActionKind> {
    vec![
        ActionKind::MarkRead(unread),
        ActionKind::SetFlagged(!flagged),
        ActionKind::Archive,
        ActionKind::MoveToTrash,
        if in_junk_folder {
            ActionKind::MarkAsNotSpam
        } else {
            ActionKind::MarkAsSpam
        },
        ActionKind::PermanentlyDelete,
    ]
}

pub(super) fn intent_for(target: MessageTarget, action: ActionKind) -> Intent {
    let MessageTarget { account, key } = target;
    match action {
        ActionKind::MarkRead(read) => Intent::MarkRead { account, key, read },
        ActionKind::SetFlagged(flagged) => Intent::SetFlagged {
            account,
            key,
            flagged,
        },
        ActionKind::Archive => Intent::Archive { account, key },
        ActionKind::MoveToTrash => Intent::Delete { account, key },
        ActionKind::MarkAsSpam => Intent::MarkAsSpam { account, key },
        ActionKind::MarkAsNotSpam => Intent::MarkAsNotSpam { account, key },
        ActionKind::PermanentlyDelete => Intent::PermanentlyDelete { account, key },
    }
}

pub(super) fn in_junk_folder(snapshot: &MailboxListSnapshot) -> bool {
    let Some(selected) = snapshot.selected.as_deref() else {
        return false;
    };
    snapshot
        .folders
        .iter()
        .find(|folder| folder.key == selected)
        .is_some_and(|folder| matches!(folder.role, Some(FolderRole::Junk)))
}

pub(super) fn message_menu_button(
    row: &FlatRow,
    in_junk_folder: bool,
    sender: &relm4::Sender<AppInput>,
) -> gtk::Box {
    let target = MessageTarget {
        account: row.account.clone(),
        key: row.key.clone(),
    };
    action_menu(
        actions_for(row.unread, row.flagged, in_junk_folder)
            .into_iter()
            .map(|action| (action_label(action), target.clone(), action)),
        sender,
    )
}

pub(super) fn thread_menu_button(
    account: &str,
    thread_id: &str,
    sender: &relm4::Sender<AppInput>,
) -> gtk::Box {
    let menu = gtk::Box::new(gtk::Orientation::Vertical, 0);
    menu.set_margin_top(6);
    menu.set_margin_bottom(6);
    menu.set_margin_start(6);
    menu.set_margin_end(6);
    let archive = gtk::Button::with_label(l10n::thread_archive());
    archive.add_css_class("flat");
    let input = sender.clone();
    let account = account.to_owned();
    let thread_id = thread_id.to_owned();
    archive.connect_clicked(move |button| {
        close_menu(button);
        input.emit(AppInput::ArchiveThread {
            account: account.clone(),
            thread_id: thread_id.clone(),
        });
    });
    menu.append(&archive);
    menu_button(&menu)
}

fn action_menu(
    actions: impl IntoIterator<Item = (&'static str, MessageTarget, ActionKind)>,
    sender: &relm4::Sender<AppInput>,
) -> gtk::Box {
    let menu = gtk::Box::new(gtk::Orientation::Vertical, 0);
    menu.set_margin_top(6);
    menu.set_margin_bottom(6);
    menu.set_margin_start(6);
    menu.set_margin_end(6);
    for (label, target, action) in actions {
        let item = gtk::Button::with_label(label);
        item.add_css_class("flat");
        if action == ActionKind::PermanentlyDelete {
            item.add_css_class("destructive-action");
        }
        let input = sender.clone();
        item.connect_clicked(move |button| {
            close_menu(button);
            if action == ActionKind::PermanentlyDelete {
                input.emit(AppInput::RequestPermanentDelete(target.clone()));
            } else {
                input.emit(AppInput::PerformMailAction(Box::new(
                    MailActionRequest::new(target.clone(), action),
                )));
            }
        });
        menu.append(&item);
    }
    menu_button(&menu)
}

fn menu_button(menu: &gtk::Box) -> gtk::Box {
    let popover = gtk::Popover::new();
    popover.set_child(Some(menu));
    let button = gtk::Button::from_icon_name("view-more-symbolic");
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

fn close_menu(button: &gtk::Button) {
    if let Some(popover) = button
        .ancestor(gtk::Popover::static_type())
        .and_downcast::<gtk::Popover>()
    {
        popover.popdown();
    }
}

fn action_label(action: ActionKind) -> &'static str {
    match action {
        ActionKind::MarkRead(true) => l10n::action_mark_read(),
        ActionKind::MarkRead(false) => l10n::action_mark_unread(),
        ActionKind::SetFlagged(true) => l10n::action_flag(),
        ActionKind::SetFlagged(false) => l10n::action_unflag(),
        ActionKind::Archive => l10n::action_archive(),
        ActionKind::MoveToTrash => l10n::action_move_to_trash(),
        ActionKind::MarkAsSpam => l10n::action_mark_as_spam(),
        ActionKind::MarkAsNotSpam => l10n::action_mark_as_not_spam(),
        ActionKind::PermanentlyDelete => l10n::action_delete_permanently(),
    }
}

#[derive(Default)]
pub(super) struct PermanentDeleteDialog {
    target: Option<DeleteTarget>,
    window: Option<gtk::Window>,
}

impl PermanentDeleteDialog {
    pub(super) fn render(
        &mut self,
        target: Option<&DeleteTarget>,
        parent: &impl IsA<gtk::Window>,
        sender: &relm4::Sender<AppInput>,
    ) {
        if self.target.as_ref() == target {
            return;
        }
        if let Some(window) = self.window.take() {
            window.close();
        }
        self.target = target.cloned();
        let Some(target) = target.cloned() else {
            return;
        };
        let window = delete_confirmation(parent, target, sender);
        window.present();
        self.window = Some(window);
    }
}

fn delete_confirmation(
    parent: &impl IsA<gtk::Window>,
    target: DeleteTarget,
    sender: &relm4::Sender<AppInput>,
) -> gtk::Window {
    let (window, _) =
        crate::ui::modal::new(parent, l10n::delete_permanently_title(), 420, Some(190));
    window.set_resizable(false);
    let content = gtk::Box::new(gtk::Orientation::Vertical, 18);
    content.set_margin_top(24);
    content.set_margin_bottom(24);
    content.set_margin_start(24);
    content.set_margin_end(24);
    let sentence = match &target {
        DeleteTarget::Message(_) => l10n::delete_permanently_message().to_owned(),
        DeleteTarget::Selection(count) => {
            l10n::delete_permanently_message_many(i64::try_from(*count).unwrap_or(i64::MAX))
        }
    };
    let message = gtk::Label::new(Some(&sentence));
    message.set_wrap(true);
    message.set_xalign(0.0);
    content.append(&message);
    let actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    actions.set_halign(gtk::Align::End);
    let cancel = gtk::Button::with_label(l10n::action_cancel());
    let dialog = window.clone();
    cancel.connect_clicked(move |_| dialog.close());
    actions.append(&cancel);
    let delete = gtk::Button::with_label(l10n::action_delete_permanently());
    delete.add_css_class("destructive-action");
    let input = sender.clone();
    let dialog = window.clone();
    delete.connect_clicked(move |_| {
        match &target {
            DeleteTarget::Message(message) => input.emit(AppInput::PerformMailAction(Box::new(
                MailActionRequest::new(message.clone(), ActionKind::PermanentlyDelete),
            ))),
            DeleteTarget::Selection(_) => {
                input.emit(AppInput::PerformSelectionAction(
                    BulkAction::PermanentlyDelete,
                ));
            }
        }
        dialog.close();
    });
    actions.append(&delete);
    content.append(&actions);
    window.set_child(Some(&content));
    let input = sender.clone();
    window.connect_close_request(move |_| {
        input.emit(AppInput::DismissPermanentDelete);
        gtk::glib::Propagation::Proceed
    });
    window
}

impl AppModel {
    pub(super) fn perform_mail_action(&mut self, request: MailActionRequest) {
        if request.action == ActionKind::PermanentlyDelete {
            self.pending_mail_delete = None;
        }
        self.dispatch(request.into_intent());
    }

    /// Runs one action over every selected row, as a single batch in the core.
    ///
    /// A permanent delete asks first, on the same terms the row menu's does; the other five are
    /// recoverable and act at once (`docs/list-selection.md`, rule 6). A move empties the
    /// selection afterwards, because the rows it named are leaving the list; a keyword edit keeps
    /// it, because the user is usually part-way through a set.
    pub(super) fn act_on_selection(&mut self, action: BulkAction) {
        if self.selection.is_empty() {
            return;
        }
        if action == BulkAction::PermanentlyDelete {
            self.pending_mail_delete = Some(DeleteTarget::Selection(
                self.selection.selected_rows().len(),
            ));
            return;
        }
        self.perform_selection(action);
    }

    /// Runs the action the user has already agreed to. The confirmation's own button is the only
    /// caller for a permanent delete: a *pending* confirmation is not consent, so
    /// [`Self::act_on_selection`] can never fall through to this one by finding the dialog slot
    /// occupied by some other row's confirmation.
    pub(super) fn perform_selection(&mut self, action: BulkAction) {
        if self.selection.is_empty() {
            return;
        }
        let rows = self.selection.selected_rows();
        self.pending_mail_delete = None;
        let removes = matches!(
            action,
            BulkAction::Archive | BulkAction::Delete | BulkAction::PermanentlyDelete
        );
        // Decided while the selection still names the rows, since the dispatch below empties it.
        let closes_reading = removes && self.selection_holds_open_message();
        self.dispatch(Intent::ActOnSelection { rows, action });
        if removes {
            self.selection.clear();
            if closes_reading {
                self.reading.close();
            }
        }
    }

    /// Whether the message in the reading pane is one of the selected rows, a conversation's
    /// members included. The pane is cleared rather than advanced: the row it would advance to
    /// may be in the same batch and about to leave too.
    fn selection_holds_open_message(&self) -> bool {
        let Some(opened) = self.reading.opened.as_ref() else {
            return false;
        };
        self.snapshot
            .rows
            .iter()
            .filter(|row| self.selection.contains(row))
            .any(|row| match row {
                SnapshotRow::Flat { row } => row.account == opened.account && row.key == opened.key,
                SnapshotRow::Thread { row } => row
                    .messages
                    .iter()
                    .any(|message| message.account == opened.account && message.key == opened.key),
            })
    }

    pub(super) fn perform_opened_mail_action(&mut self, action: ActionKind) {
        if !matches!(action, ActionKind::Archive | ActionKind::MoveToTrash) {
            return;
        }
        let Some(opened) = self.reading.opened.clone() else {
            return;
        };
        let stops = model::readable_stops(&self.snapshot, &self.expanded_threads);
        let next = model::message_after_removing(&opened, &stops);
        self.dispatch(intent_for(
            MessageTarget {
                account: opened.account,
                key: opened.key,
            },
            action,
        ));
        if let Some(next) = next {
            self.open_message(next);
        } else {
            self.reading.close();
        }
    }

    /// Records a conversation's inline disclosure and, on opening one, reads its representative
    /// message; the three-pane behaviour macOS and Windows share. Collapsing leaves the reading
    /// pane where it is: the user is closing a list row, not the message they are reading.
    pub(super) fn set_thread_expanded(&mut self, thread: &ThreadKey, expanded: bool) {
        if !expanded {
            self.expanded_threads.remove(thread);
            return;
        }
        self.expanded_threads.insert(thread.clone());
        if let Some(message) = mailbox::thread_representative(&self.snapshot, thread) {
            self.open_message(message);
        }
    }

    pub(super) fn retry_open(&self) {
        if let Some(opened) = &self.reading.opened {
            self.dispatch(Intent::OpenMessage {
                account: opened.account.clone(),
                key: opened.key.clone(),
            });
        }
    }

    pub(super) fn archive_thread(&mut self, account: &str, thread_id: &str) {
        let contains_opened = self.reading.opened.as_ref().is_some_and(|opened| {
            self.snapshot.rows.iter().any(|row| match row {
                SnapshotRow::Thread { row }
                    if row.account == account && row.thread_id == thread_id =>
                {
                    row.messages.iter().any(|message| {
                        message.account == opened.account && message.key == opened.key
                    })
                }
                _ => false,
            })
        });
        self.expanded_threads
            .remove(&mailbox::ThreadKey::new(account, thread_id));
        self.dispatch(Intent::ArchiveThread {
            account: account.to_owned(),
            thread_id: thread_id.to_owned(),
        });
        if contains_opened {
            self.reading.close();
        }
    }
}

#[cfg(test)]
#[path = "mail_actions_tests.rs"]
pub(crate) mod tests;
