//! The rows a destination picker draws: Move to… on a folder, and Move to folder… on a message
//! (`docs/folder-pane.md`, rule 24).
//!
//! Both pickers draw the account's tree as the pane does, so the rows are decided here once, as
//! plain values tested without a display; `folder_dialogs` only lays them out. Which folders are
//! destinations is each picker's own decision, read from the flags the core stamps (rule 22).

use std::collections::HashSet;

use mailcal_bindings::FolderRow;

use super::{
    folder_actions::path_label, folder_names::folder_label, folder_pane_rows::role_icon, icons,
};
use crate::l10n;

/// One folder as a picker draws it, before anyone has said whether it is a destination.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PickerFolder {
    pub(crate) key: String,
    /// The folder it is drawn under in the pane, or `None` at the top of the tree.
    pub(crate) parent: Option<String>,
    pub(crate) depth: u32,
    /// Its own name, in the user's words.
    pub(crate) name: String,
    /// The icon the pane draws for its role.
    pub(crate) icon: &'static str,
    /// Its path, `Clients / Acme`, in the user's words.
    pub(crate) path: String,
}

/// Every folder of one account, in the pane's order.
pub(crate) fn picker_folders(folders: &[FolderRow]) -> Vec<PickerFolder> {
    folders
        .iter()
        .map(|folder| PickerFolder {
            key: folder.key.clone(),
            parent: folder.parent.clone(),
            depth: folder.depth,
            name: folder_label(folder.role.as_ref(), &folder.name),
            icon: role_icon(folder.role.as_ref()),
            path: path_label(folders, folder),
        })
        .collect()
}

/// One row of a picker.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PickerRow {
    /// The folder the row stands for, or `None` for Top level.
    pub(crate) key: Option<String>,
    /// What the row shows: the folder's own name, or Top level's word.
    pub(crate) name: String,
    /// The pane's icon for the folder's role; the account's icon on Top level.
    pub(crate) icon: &'static str,
    /// How many of the pane's indent steps the row sits in.
    pub(crate) indent: u32,
    /// Whether choosing the row moves anything there. A row that is not a destination is only
    /// there because a destination sits inside it.
    pub(crate) enabled: bool,
    /// What assistive technology reads, since an indent is not spoken: the folder's path.
    pub(crate) label: String,
}

/// The rows a picker draws for `folders`, in the pane's order.
///
/// A folder in `destinations` is an enabled row. A folder that is not one is drawn, disabled,
/// only when a destination sits somewhere inside it, so every destination keeps the parent it
/// has in the pane; any other folder is left out. With `top_level`, a Top level row comes first,
/// enabled as it says, and every folder sits one step inside it, as the pane roots each tree at
/// its account. A Top level that is not enabled is drawn only as the root of a folder that is.
pub(crate) fn picker_rows(
    folders: &[PickerFolder],
    destinations: &HashSet<String>,
    top_level: Option<bool>,
) -> Vec<PickerRow> {
    let mut kept: HashSet<&str> = HashSet::new();
    for folder in folders.iter().filter(|f| destinations.contains(&f.key)) {
        kept.insert(&folder.key);
        let mut parent = folder.parent.as_deref();
        // A key already kept has its whole chain kept with it, so the walk stops there; which
        // also ends it on a chain that loops.
        while let Some(key) = parent {
            if !kept.insert(key) {
                break;
            }
            parent = folders
                .iter()
                .find(|f| f.key == key)
                .and_then(|f| f.parent.as_deref());
        }
    }
    let step = u32::from(top_level.is_some());
    let top = top_level
        .filter(|&enabled| enabled || !kept.is_empty())
        .map(|enabled| PickerRow {
            key: None,
            name: l10n::folder_move_top_level().to_owned(),
            icon: icons::ACCOUNT,
            indent: 0,
            enabled,
            label: l10n::folder_move_top_level().to_owned(),
        });
    top.into_iter()
        .chain(
            folders
                .iter()
                .filter(|folder| kept.contains(folder.key.as_str()))
                .map(|folder| PickerRow {
                    key: Some(folder.key.clone()),
                    name: folder.name.clone(),
                    icon: folder.icon,
                    indent: folder.depth + step,
                    enabled: destinations.contains(&folder.key),
                    label: folder.path.clone(),
                }),
        )
        .collect()
}
