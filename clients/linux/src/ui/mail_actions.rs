//! What a mail action is, what it dispatches, and the confirmation the irreversible one needs.
//!
//! The menus a row offers them through are [`super::mail_actions_menu`].

use adw::prelude::*;
use mailcal_bindings::{BulkAction, Intent, SelectedRow, SnapshotRow};

use super::{AppInput, AppModel, mailbox, mailbox::ThreadKey, model, reader::ReadingSource};
use crate::l10n;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct MessageTarget {
    pub(crate) account: String,
    pub(crate) key: String,
}

/// What the "delete permanently?" confirmation is about to destroy.
///
/// Every path asks, because each is irreversible; only the sentence differs, and the arms over
/// several rows carry them so it can say how much is going (`docs/list-selection.md`, rule 6).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum DeleteTarget {
    /// One message, named by a row menu's item.
    Message(MessageTarget),
    /// Every selected row, and how many there are.
    Selection(usize),
    /// The rows a row menu was opened on, outside the selection.
    Rows(Vec<SelectedRow>),
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
        DeleteTarget::Selection(count) => many(*count),
        DeleteTarget::Rows(rows) => many(rows.len()),
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
            DeleteTarget::Rows(rows) => input.emit(AppInput::PerformRowsAction {
                rows: rows.clone(),
                action: BulkAction::PermanentlyDelete,
            }),
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

fn many(count: usize) -> String {
    l10n::delete_permanently_message_many(i64::try_from(count).unwrap_or(i64::MAX))
}

impl AppModel {
    /// A write on the one message a row menu's item named. A permanent delete arrives here only
    /// from its confirmation's own button.
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
        // Decided while the selection still names the rows, since the dispatch below empties it.
        if self.perform_rows(rows, action) {
            self.selection.clear();
        }
    }

    /// Runs one action the user has already agreed to over `rows`, as a single batch in the core,
    /// and answers whether it takes them out of the list. A conversation leaving folds away, and
    /// the reading pane lets go of a message leaving with it.
    pub(super) fn perform_rows(&mut self, rows: Vec<SelectedRow>, action: BulkAction) -> bool {
        self.pending_mail_delete = None;
        let removes = matches!(
            action,
            BulkAction::Archive | BulkAction::Delete | BulkAction::PermanentlyDelete
        );
        if removes {
            if self.rows_hold_open_message(&rows) {
                self.reading.close();
            }
            for row in &rows {
                if let SelectedRow::Thread { account, thread_id } = row {
                    self.expanded_threads
                        .remove(&ThreadKey::new(account, thread_id));
                }
            }
        }
        self.dispatch(Intent::ActOnSelection { rows, action });
        removes
    }

    /// Whether the message in the reading pane is one of the selected rows, a conversation's
    /// members included.
    pub(super) fn selection_holds_open_message(&self) -> bool {
        self.rows_hold_open_message(&self.selection.selected_rows())
    }

    /// Whether the message in the reading pane is one of `rows`, a conversation's members
    /// included. The pane is cleared rather than advanced: the row it would advance to may be in
    /// the same batch and about to leave too.
    fn rows_hold_open_message(&self, rows: &[SelectedRow]) -> bool {
        let Some(opened) = self.reading.opened.as_ref() else {
            return false;
        };
        self.snapshot
            .rows
            .iter()
            .filter(|row| rows.contains(&super::selection::RowKey::of(row).selected_row()))
            .any(|row| match row {
                SnapshotRow::Flat { row } => row.account == opened.account && row.key == opened.key,
                SnapshotRow::Thread { row } => row
                    .messages
                    .iter()
                    .any(|message| message.account == opened.account && message.key == opened.key),
            })
    }

    /// Archive or Trash from a reading view's own action row.
    ///
    /// What is left behind differs by reader, and deliberately so (`docs/reading-window.md`): the
    /// pane is a place in a list, so it advances to the next message down; a window is one
    /// message, so once that message has left the folder there is nothing for the window to be
    /// about and it closes.
    pub(super) fn perform_opened_mail_action(
        &mut self,
        source: &ReadingSource,
        action: ActionKind,
    ) {
        if !matches!(action, ActionKind::Archive | ActionKind::MoveToTrash) {
            return;
        }
        let Some(opened) = self.reader_message(source).cloned() else {
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
        match source {
            ReadingSource::Pane => {
                if let Some(next) = next {
                    self.open_message(next);
                } else {
                    self.reading.close();
                }
            }
            ReadingSource::Window(window) => self.close_reading_window(&window.clone()),
        }
    }

    /// Records a conversation's inline disclosure and, on opening one, opens its newest message
    /// (`docs/row-actions.md`, rule 8). Collapsing leaves the reading pane where it is: the user
    /// is closing a list row, not the message they are reading.
    ///
    /// A disclosure the model already holds changes nothing: the list draws the model's expansion
    /// back onto its rows, and the row reports that change too.
    pub(super) fn set_thread_expanded(
        &mut self,
        thread: &ThreadKey,
        expanded: bool,
        sender: &relm4::Sender<AppInput>,
    ) {
        if !expanded {
            self.expanded_threads.remove(thread);
        } else if !self.expanded_threads.contains(thread) {
            self.open_conversation(thread, sender);
        }
    }

    /// Expands a conversation and opens its newest message, resuming it when it is a draft.
    pub(super) fn open_conversation(
        &mut self,
        thread: &ThreadKey,
        sender: &relm4::Sender<AppInput>,
    ) {
        self.expanded_threads.insert(thread.clone());
        if let Some((message, draft)) = mailbox::thread_representative(&self.snapshot, thread) {
            self.open_or_resume(message, draft, sender);
        }
    }

    /// Asks for the body again, into the slot the reader that asked already owns: a window's
    /// retry is the pane's retry aimed elsewhere, never a fetch of its own
    /// (`docs/reading-window.md`).
    pub(super) fn retry_open(&self, source: &ReadingSource) {
        let Some(opened) = self.reader_message(source) else {
            return;
        };
        let account = opened.account.clone();
        let key = opened.key.clone();
        self.dispatch(match source.window() {
            None => Intent::OpenMessage { account, key },
            Some(window) => Intent::OpenMessageInWindow {
                window: window.to_owned(),
                account,
                key,
            },
        });
    }
}

#[cfg(test)]
#[path = "mail_actions_tests.rs"]
pub(crate) mod tests;
