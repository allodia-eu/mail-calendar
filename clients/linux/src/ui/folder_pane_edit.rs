//! What makes a pane row changeable: its menu, the drag it starts and the drops it takes, and the
//! look of a row still waiting for the server (`docs/folder-pane.md`, "Changing the tree").
//!
//! Split from `folder_pane_rows`, which draws a row, so both stay under the line limit. Every
//! decision is `folder_actions`'; this file only attaches it.

use adw::prelude::*;
use gtk::accessible::Property as AccessibleProperty;
use mailcal_bindings::{FolderIntent, FolderNotice, FolderProblem, FolderRow};

use super::{
    AppInput,
    folder_actions::{
        Dragged, DropSpot, FolderInput, FolderMenuItem, NameCheck, account_menu, folder_menu,
        move_candidates,
    },
    folder_dialogs::{NamePurpose, delete_dialog, move_dialog, name_dialog},
    folder_drag::{self, PaneDrag},
    folder_menu,
    folder_names::folder_label,
};
use crate::l10n;

/// Gives a folder row its menu, drag and drop, or, while it waits for the server, only the look
/// that says so (rule 27).
pub(super) fn folder(
    row: &adw::ActionRow,
    account: &str,
    folder: &FolderRow,
    folders: &[FolderRow],
    check: &NameCheck,
    sender: &relm4::Sender<AppInput>,
) {
    if folder.pending {
        row.add_css_class("dim-label");
        row.set_tooltip_text(Some(l10n::folder_pending()));
        row.upcast_ref::<gtk::Widget>()
            .update_property(&[AccessibleProperty::Description(l10n::folder_pending())]);
        return;
    }
    let name = folder_label(folder.role.as_ref(), &folder.name);
    let candidates = if folder.editable {
        move_candidates(folders, &folder.key)
    } else {
        Vec::new()
    };
    let (account_id, key, parent, in_trash) = (
        account.to_owned(),
        folder.key.clone(),
        folder.parent.clone(),
        folder.in_trash,
    );
    let (check, input) = (check.clone(), sender.clone());
    folder_menu::attach(row, folder_menu(folder), move |item, anchor| match item {
        FolderMenuItem::NewFolder => name_dialog(
            anchor,
            &account_id,
            NamePurpose::Create {
                parent: Some(key.clone()),
            },
            &check,
            &input,
        ),
        FolderMenuItem::Rename => name_dialog(
            anchor,
            &account_id,
            NamePurpose::Rename {
                key: key.clone(),
                parent: parent.clone(),
                current: name.clone(),
            },
            &check,
            &input,
        ),
        FolderMenuItem::MoveTo => {
            move_dialog(anchor, &account_id, &key, &name, candidates.clone(), &input);
        }
        FolderMenuItem::Delete => delete_dialog(anchor, &account_id, &key, &name, in_trash, &input),
    });
    if folder.editable {
        folder_drag::source(
            row,
            PaneDrag {
                account: account.to_owned(),
                dragged: Dragged::Folder {
                    key: folder.key.clone(),
                    parent: folder.parent.clone(),
                },
            },
        );
    }
    let (into, input) = (folder.key.clone(), sender.clone());
    folder_drag::target(row, account, DropSpot::of(folder, folders), move |drag| {
        input.emit(AppInput::Folder(dropped(drag, Some(into.clone()))));
    });
}

/// Gives an account row New folder and makes it take a folder dropped there, which moves it to the
/// top of the tree.
pub(super) fn account(
    row: &adw::ActionRow,
    account: &str,
    manages_folders: bool,
    check: &NameCheck,
    sender: &relm4::Sender<AppInput>,
) {
    let (account_id, check, input) = (account.to_owned(), check.clone(), sender.clone());
    folder_menu::attach(row, account_menu(manages_folders), move |_, anchor| {
        name_dialog(
            anchor,
            &account_id,
            NamePurpose::Create { parent: None },
            &check,
            &input,
        );
    });
    let input = sender.clone();
    folder_drag::target(
        row,
        account,
        DropSpot::Account { manages_folders },
        move |drag| input.emit(AppInput::Folder(dropped(drag, None))),
    );
}

/// What a drop onto `into` (`None`: the account row) asks of the model.
fn dropped(drag: PaneDrag, into: Option<String>) -> FolderInput {
    match drag.dragged {
        Dragged::Folder { key, .. } => FolderInput::Change(FolderIntent::Move {
            account: drag.account,
            key,
            parent: into,
        }),
        Dragged::Mail(row) => FolderInput::DropMail {
            row,
            account: drag.account,
            key: into.unwrap_or_default(),
        },
    }
}

/// The sentence a refused folder change stands on the pane as (rule 28).
pub(crate) fn notice_text(notice: &FolderNotice) -> String {
    match notice.problem {
        FolderProblem::ChangedElsewhere => l10n::folder_notice_changed(&notice.folder),
        FolderProblem::Refused => l10n::folder_notice_refused(&notice.folder),
    }
}

/// The banner above the pane's tree that carries a refused folder change until it is dismissed.
pub(crate) fn notice_banner(sender: &relm4::Sender<AppInput>) -> adw::Banner {
    let banner = adw::Banner::new("");
    // A folder name is the server's text: markup-shaped input must render as itself.
    banner.set_use_markup(false);
    banner.set_button_label(Some(l10n::action_close()));
    let input = sender.clone();
    banner.connect_button_clicked(move |_| {
        input.emit(AppInput::Folder(FolderInput::Change(
            FolderIntent::DismissNotice,
        )));
    });
    banner
}

pub(crate) fn render_notice(banner: &adw::Banner, notice: Option<&FolderNotice>) {
    banner.set_title(&notice.map(notice_text).unwrap_or_default());
    banner.set_revealed(notice.is_some());
}
