//! What the model does with a row menu's pick (`docs/row-actions.md`), and the one conversation
//! a selection keeps expanded (rule 8).

use mailcal_bindings::{BulkAction, MenuMessage, MessageKind, RowMenu};

use super::{
    AppInput, AppModel,
    composer_model::ComposeKind,
    composer_open::{ComposerTarget, answer_origin},
    mail_actions::DeleteTarget,
    model,
    notification_open::find_in_list,
    row_menu::{BarActions, RowPick},
    selection::SelectMode,
};

impl AppModel {
    /// A click on the list row at `index`. Leaving a conversation folds it away.
    pub(super) fn select_row(&mut self, index: usize, mode: SelectMode) {
        self.selection.click(&self.snapshot.rows, index, mode);
        self.expanded_threads =
            model::kept_expanded(&self.expanded_threads, &self.selection.selected_rows());
    }

    pub(super) fn perform_row_pick(&mut self, pick: RowPick, sender: &relm4::Sender<AppInput>) {
        match pick {
            RowPick::Open(message) => self.open_menu_target(&message, sender),
            RowPick::OpenInWindow(message) => {
                if let Some(found) = find_in_list(&self.snapshot, &message.account, &message.key) {
                    self.open_reading_window(found.message);
                }
            }
            RowPick::Answer(kind, message) => self.answer(kind, message, sender),
            RowPick::Message(request) => self.perform_mail_action(request),
            RowPick::DeleteMessage(target) => {
                self.pending_mail_delete = Some(DeleteTarget::Message(target));
            }
            RowPick::Rows { rows, action } => {
                // The menu over the selection runs the bar's own path, which asks before a
                // permanent delete and lets go of the rows a move takes.
                if rows == self.selection.selected_rows() {
                    self.act_on_selection(action);
                } else if action == BulkAction::PermanentlyDelete {
                    self.pending_mail_delete = Some(DeleteTarget::Rows(rows));
                } else {
                    self.perform_rows(rows, action);
                }
            }
            // Asked for in the menu itself, which owns the folder picker.
            RowPick::Move => {}
        }
    }

    /// Opens the message an item names as selecting its row would: a conversation's message by
    /// selecting and expanding that conversation onto it, a draft into its composer.
    fn open_menu_target(&mut self, target: &MenuMessage, sender: &relm4::Sender<AppInput>) {
        let Some(found) = find_in_list(&self.snapshot, &target.account, &target.key) else {
            return;
        };
        self.select_row(found.index, SelectMode::Replace);
        if let Some(thread) = found.thread {
            self.expanded_threads.insert(thread);
        }
        self.open_or_resume(found.message, target.kind == MessageKind::Draft, sender);
    }

    fn answer(&self, kind: ComposeKind, message: MenuMessage, sender: &relm4::Sender<AppInput>) {
        let origin = answer_origin(kind, message.account, message.key);
        self.open_composer_on(origin, None, ComposerTarget::Pane, Vec::new(), sender);
    }

    /// What the selection bar offers: the core's menu over the selection, or nothing to offer
    /// while nothing is picked.
    pub(super) fn bar_actions(&self) -> BarActions {
        let menu = match &self.app {
            Some(app) if !self.selection.is_empty() => app.row_menu(self.selection.selected_rows()),
            _ => RowMenu::default(),
        };
        BarActions::of(&menu)
    }
}
