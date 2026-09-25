//! Conversions for the folder-tree surface: the FFI [`FolderIntent`] into the core's, and the
//! pane's notice and name check back out. Split from `convert` to keep each file under the
//! 500-line limit.

use engine_api::AccountId;
use mailcal_app::{FolderIntent as AppFolderIntent, FolderRef, MessageRef, RowRef, ThreadRef};
use mailcal_viewmodel::{
    FolderAction as AppFolderAction, FolderNameCheck as AppFolderNameCheck,
    FolderNotice as AppFolderNotice, FolderProblem as AppFolderProblem,
};

use crate::{
    FolderAction, FolderIntent, FolderNameCheck, FolderNotice, FolderProblem, SelectedRow,
};

/// Binds every folder action's account and key into one [`FolderRef`] at the boundary, as
/// `Intent::SelectFolder` does, so no change can name a key without its account. A malformed
/// reference drops the whole intent: moving three of four dropped rows is worse than none.
pub(crate) fn folder_intent(intent: FolderIntent) -> Result<AppFolderIntent, String> {
    let folder = |account: &str, key: String| {
        FolderRef::from_parts(account, key).ok_or_else(|| "invalid folder reference".to_owned())
    };
    Ok(match intent {
        FolderIntent::Create {
            account,
            parent,
            name,
        } => AppFolderIntent::Create {
            account: AccountId::try_from(account.as_str())
                .map_err(|_| "invalid account id".to_owned())?,
            parent,
            name,
        },
        FolderIntent::Rename { account, key, name } => AppFolderIntent::Rename {
            folder: folder(&account, key)?,
            name,
        },
        FolderIntent::Move {
            account,
            key,
            parent,
        } => AppFolderIntent::Move {
            folder: folder(&account, key)?,
            parent,
        },
        FolderIntent::Delete { account, key } => AppFolderIntent::Delete(folder(&account, key)?),
        FolderIntent::MoveMessages { rows, account, key } => AppFolderIntent::MoveMessages {
            rows: rows
                .into_iter()
                .map(row_ref)
                .collect::<Result<Vec<_>, _>>()?,
            folder: folder(&account, key)?,
        },
        FolderIntent::DismissNotice => AppFolderIntent::DismissNotice,
    })
}

fn row_ref(row: SelectedRow) -> Result<RowRef, String> {
    match row {
        SelectedRow::Message { account, key } => MessageRef::from_parts(&account, key)
            .map(RowRef::Message)
            .ok_or_else(|| "invalid message reference".to_owned()),
        SelectedRow::Thread { account, thread_id } => ThreadRef::from_parts(&account, thread_id)
            .map(RowRef::Thread)
            .ok_or_else(|| "invalid thread reference".to_owned()),
    }
}

impl From<AppFolderNotice> for FolderNotice {
    fn from(notice: AppFolderNotice) -> Self {
        Self {
            account: notice.account,
            folder: notice.folder,
            action: notice.action.into(),
            problem: notice.problem.into(),
        }
    }
}

impl From<AppFolderAction> for FolderAction {
    fn from(action: AppFolderAction) -> Self {
        match action {
            AppFolderAction::Create => Self::Create,
            AppFolderAction::Rename => Self::Rename,
            AppFolderAction::Move => Self::Move,
            AppFolderAction::Delete => Self::Delete,
        }
    }
}

impl From<AppFolderProblem> for FolderProblem {
    fn from(problem: AppFolderProblem) -> Self {
        match problem {
            AppFolderProblem::ChangedElsewhere => Self::ChangedElsewhere,
            AppFolderProblem::Refused => Self::Refused,
        }
    }
}

impl From<AppFolderNameCheck> for FolderNameCheck {
    fn from(check: AppFolderNameCheck) -> Self {
        match check {
            AppFolderNameCheck::Valid => Self::Valid,
            AppFolderNameCheck::Empty => Self::Empty,
            AppFolderNameCheck::Surrounded => Self::Surrounded,
            AppFolderNameCheck::Control => Self::Control,
            AppFolderNameCheck::Separator => Self::Separator,
            AppFolderNameCheck::Taken => Self::Taken,
        }
    }
}

#[cfg(test)]
#[path = "convert_folders_tests.rs"]
mod tests;
