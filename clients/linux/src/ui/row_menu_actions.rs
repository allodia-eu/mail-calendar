//! What the model does with a row menu's pick (`docs/row-actions.md`), and the one conversation
//! a selection keeps expanded (rule 8).

use mailcal_bindings::{
    Command, CommandBar, CommandItem, Intent, MenuMessage, MessageKind, RowAction,
};

use super::{
    AppInput, AppModel,
    composer_model::ComposeKind,
    composer_open::{ComposerTarget, answer_origin},
    mail_actions::DeleteTarget,
    model,
    notification_open::find_in_list,
    row_menu::{self, RowPick},
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
            RowPick::Open(message) => {
                self.open_as_clicked(&message, sender);
            }
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
            // The rows the menu was opened on, the selection or its own row alone, as they were
            // when it opened (`docs/list-selection.md`, rule 12).
            RowPick::Rows { rows, action } => self.act_on_rows(rows, action),
            // Asked for in the menu itself, which owns the folder picker.
            RowPick::Move => {}
        }
    }

    /// Opens a message the core named as selecting its row would: a conversation's message by
    /// selecting and expanding that conversation onto it, a draft into its composer. Answers
    /// whether the list still shows it.
    pub(super) fn open_as_clicked(
        &mut self,
        target: &MenuMessage,
        sender: &relm4::Sender<AppInput>,
    ) -> bool {
        let Some(found) = find_in_list(&self.snapshot, &target.account, &target.key) else {
            return false;
        };
        self.select_row(found.index, SelectMode::Replace);
        if let Some(thread) = found.thread {
            self.expanded_threads.insert(thread);
        }
        self.open_or_resume(found.message, target.kind == MessageKind::Draft, sender);
        true
    }

    fn answer(&self, kind: ComposeKind, message: MenuMessage, sender: &relm4::Sender<AppInput>) {
        let origin = answer_origin(kind, message.account, message.key);
        self.open_composer_on(origin, None, ComposerTarget::Pane, Vec::new(), sender);
    }

    /// The command bar over the selection, as the core answers it (`docs/command-bar.md`, rule
    /// 1). Asked again only when the selection or the published list has changed.
    pub(super) fn command_bar(&self) -> CommandBar {
        self.bar_answer.answer(
            self.snapshot.generation(),
            self.selection.selected_rows(),
            |rows| {
                self.app
                    .as_ref()
                    .map_or_else(CommandBar::default, |app| app.command_bar(rows))
            },
        )
    }

    /// A bar item picked: run as the same item in a row's menu over the selection runs, asking
    /// first where the core says so. Sync is the one item no menu carries.
    pub(super) fn perform_bar_pick(
        &mut self,
        item: &CommandItem,
        sender: &relm4::Sender<AppInput>,
    ) {
        if !item.enabled {
            return;
        }
        let Some(menu_item) = row_menu::as_menu_item(item) else {
            self.dispatch(Intent::RefreshMail);
            return;
        };
        if let Some(pick) = row_menu::pick(&menu_item, &self.selection.selected_rows()) {
            self.perform_row_pick(pick, sender);
        }
    }

    /// The Delete key over the selection: the bar's Delete, which in Trash is a permanent delete
    /// that asks first (`docs/list-selection.md`, rule 9).
    pub(super) fn delete_selection(&mut self, sender: &relm4::Sender<AppInput>) {
        if let Some(item) = delete_item(&self.command_bar()) {
            self.perform_bar_pick(&item, sender);
        }
    }
}

/// The bar's Delete, or Delete permanently in its place.
pub(super) fn delete_item(bar: &CommandBar) -> Option<CommandItem> {
    bar.sections
        .iter()
        .flat_map(|section| &section.items)
        .find(|item| {
            matches!(
                item.command,
                Command::Act {
                    action: RowAction::Delete | RowAction::PermanentlyDelete
                }
            )
        })
        .cloned()
}
