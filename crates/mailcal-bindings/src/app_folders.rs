//! Folder-tree FFI methods on [`MailcalApp`] that answer rather than act: the changes
//! themselves are `Intent::Folders`. Split out of `lib.rs` to keep each file under the
//! 500-line limit.

use engine_api::AccountId;

use crate::{FolderNameCheck, MailcalApp};

#[uniffi::export]
impl MailcalApp {
    /// Whether `name` can be given to a folder inside `parent` (a folder key of `account`'s, or
    /// `None` for the top level), for a name dialog to call as the user types. `renaming` is the
    /// key of the folder being renamed, which may keep its own name.
    ///
    /// Enable the dialog's confirm button only on `Valid`; every other answer has a line of its
    /// own in the catalog (`folder_name_*`). A malformed account id answers `Empty`, which a
    /// host that passes a row's own account never meets.
    pub fn check_folder_name(
        &self,
        account: String,
        parent: Option<String>,
        name: String,
        renaming: Option<String>,
    ) -> FolderNameCheck {
        let Ok(account) = AccountId::try_from(account.as_str()) else {
            return FolderNameCheck::Empty;
        };
        self.runtime
            .block_on(self.app.check_folder_name(
                &account,
                parent.as_deref(),
                &name,
                renaming.as_deref(),
            ))
            .into()
    }
}
