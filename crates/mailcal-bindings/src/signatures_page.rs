//! Putting Settings → Signatures together from what the core holds (`docs/signatures.md`,
//! `docs/waiting.md`): while a change to the library runs or waits, every control on the page is
//! held, and the one that started it runs.

use mailcal_viewmodel::{
    AccountSignatureRow, SignatureChangeKind as AppChangeKind,
    SignatureImageRefusal as AppImageRefusal, SignatureOutcome as AppOutcome,
    SignatureRefusal as AppRefusal, SignatureResult as AppResult, SignaturesView,
};

use crate::{
    AccountSignatures, ControlState, SignatureChangeKind, SignatureEntry, SignatureImageRefusal,
    SignatureOutcome, SignaturePicker, SignatureRefusal, SignatureResult, SignatureSlotKind,
    SignaturesPage,
};

/// The page for `view`.
pub(crate) fn page(view: SignaturesView) -> SignaturesPage {
    let busy = !view.work.is_empty();
    let held = ControlState {
        enabled: !busy,
        running: false,
    };
    let runs = |kind: AppChangeKind, id: Option<&str>| {
        view.work
            .iter()
            .any(|work| work.kind == kind && id.is_none_or(|id| work.id == id))
    };
    let running = |running: bool| ControlState {
        enabled: !busy,
        running,
    };
    let picker = |selected: Option<String>| SignaturePicker {
        selected,
        control: held,
    };
    SignaturesPage {
        signatures: view
            .library
            .iter()
            .map(|row| SignatureEntry {
                id: row.id.clone(),
                name: row.name.clone(),
                edit: held,
                delete: running(runs(AppChangeKind::Delete, Some(&row.id))),
            })
            .collect(),
        accounts: view
            .accounts
            .into_iter()
            .map(|row: AccountSignatureRow| AccountSignatures {
                account_id: row.account_id,
                email: row.email,
                new_message: picker(row.new_message),
                reply_forward: picker(row.reply_forward),
            })
            .collect(),
        add: held,
        new_id: view.new_id,
        save: running(runs(AppChangeKind::Save, None)),
        outcome: view.outcome.map(outcome),
    }
}

fn outcome(outcome: AppOutcome) -> SignatureOutcome {
    SignatureOutcome {
        serial: outcome.serial,
        id: outcome.id,
        kind: match outcome.kind {
            AppChangeKind::Save => SignatureChangeKind::Save,
            AppChangeKind::Delete => SignatureChangeKind::Delete,
        },
        result: match outcome.result {
            AppResult::Saved => SignatureResult::Saved,
            AppResult::Deleted => SignatureResult::Deleted,
            AppResult::Refused { reason } => SignatureResult::Refused {
                reason: reason.into(),
            },
            AppResult::NotWritten => SignatureResult::NotWritten,
        },
    }
}

impl From<AppRefusal> for SignatureRefusal {
    fn from(refusal: AppRefusal) -> Self {
        match refusal {
            AppRefusal::BlankName => Self::BlankName,
            AppRefusal::Gone => Self::Gone,
            AppRefusal::Image { refusal } => Self::Image {
                refusal: refusal.into(),
            },
        }
    }
}

impl From<AppImageRefusal> for SignatureImageRefusal {
    fn from(refusal: AppImageRefusal) -> Self {
        match refusal {
            AppImageRefusal::Format => Self::Format,
            AppImageRefusal::TooLarge { limit_bytes } => Self::TooLarge { limit_bytes },
        }
    }
}

impl From<SignatureSlotKind> for mailcal_viewmodel::SignatureSlotKind {
    fn from(slot: SignatureSlotKind) -> Self {
        match slot {
            SignatureSlotKind::NewMessage => Self::NewMessage,
            SignatureSlotKind::ReplyForward => Self::ReplyForward,
        }
    }
}

impl From<mailcal_app::SignatureBody> for crate::SignatureBody {
    fn from(body: mailcal_app::SignatureBody) -> Self {
        Self {
            id: body.id,
            body_html: body.body_html,
            body_plain: body.body_plain,
        }
    }
}

#[cfg(test)]
#[path = "signatures_page_tests.rs"]
mod tests;
