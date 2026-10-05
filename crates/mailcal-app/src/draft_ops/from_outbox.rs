//! Moving a message out of the Outbox and back into Drafts, which is what Edit on a queued send
//! does.
//!
//! Once the user presses Send the message lives in one place, the Outbox, and nowhere else
//! (`docs/sending.md`). Edit is the user moving it back: it becomes a draft again, opened in a
//! composer that saves over that draft, holding every file it had. The order is what keeps it
//! from being lost on the way: the files are staged and the draft is saved, durably, before the
//! send leaves the queue. An app that ends between the two leaves the message in both places,
//! never in neither.

use std::path::Path;

use engine_api::{AccountId, Draft, DraftAttachmentDisposition, PendingOpId, Provider};

use super::{Composition, Threading};
use crate::{App, ComposeRequest, CompositionId, protocol::StagedAttachment};

/// Rising counter for the staged files' names, as for a forward's.
static STAGE_SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Why a queued send could not be moved back into Drafts. It stays in the Outbox either way.
#[derive(Debug)]
pub(crate) enum NotMoved {
    /// It carries what a composer cannot hold (an invitation's answer), so editing it would
    /// send something else.
    NotEditable,
    /// Its files could not be written where the composer reads them.
    Staging(String),
    /// The draft could not be saved, nor queued to be.
    Save(String),
}

impl NotMoved {
    /// Why, in the words a log line uses.
    pub(crate) fn words(&self) -> String {
        match self {
            Self::NotEditable => {
                "it answers an invitation, which a composer cannot hold".to_owned()
            }
            Self::Staging(err) => {
                format!("its files could not be prepared for the composer: {err}")
            }
            Self::Save(err) => format!("it could not be saved as a draft: {err}"),
        }
    }
}

impl<P: Provider> App<P> {
    /// Saves `draft` as a draft of a new composition, staging its files into `staging`, and
    /// answers with the request a host opens its composer from.
    ///
    /// The caller withdraws the send only after this succeeded, and calls
    /// [`discard_draft`](Self::discard_draft) on the composition it names if the withdrawal is
    /// refused, so the message is never in neither place.
    pub(crate) async fn move_to_drafts(
        &self,
        account: &AccountId,
        op: PendingOpId,
        draft: &Draft,
        staging: &str,
    ) -> Result<ComposeRequest, NotMoved> {
        if draft.calendar.is_some() {
            return Err(NotMoved::NotEditable);
        }
        let attachments = stage_draft_files(draft, staging).map_err(NotMoved::Staging)?;
        let Some(composition) = CompositionId::new(format!("outbox-{}-{}", op.get(), mint()))
        else {
            return Err(NotMoved::Save("could not name the composition".to_owned()));
        };
        self.drafts
            .lock()
            .expect("drafts mutex poisoned")
            .open
            .insert(
                composition.clone(),
                Composition {
                    account: account.clone(),
                    message_id: draft.message_id.clone(),
                    key: None,
                    saved: None,
                    queued: None,
                    threading: draft.in_reply_to.clone().map(|in_reply_to| Threading {
                        in_reply_to,
                        references: draft.references.clone(),
                    }),
                },
            );
        match self.put_draft(account, draft, None).await {
            // No digest: the composer's first save renders its own document, which is not
            // byte for byte this one, so it must write.
            Some(Ok(key)) => self.adopt_saved_key(&composition, key),
            Some(Err(err)) => {
                let Some(queued) = self.queued_save(account, draft).await else {
                    self.close_composition(&composition);
                    return Err(NotMoved::Save(err.to_string()));
                };
                self.record_queued(&composition, queued);
            }
            None => {
                self.close_composition(&composition);
                return Err(NotMoved::Save(
                    "the account has no provider to save through".to_owned(),
                ));
            }
        }
        Ok(ComposeRequest {
            account: account.as_str().to_owned(),
            composition: composition.as_str().to_owned(),
            to: join(&draft.to),
            cc: join(&draft.cc),
            bcc: join(&draft.bcc),
            subject: draft.subject.clone(),
            body_text: draft.text_body.clone(),
            attachments,
        })
    }

    /// Applies the composition's threading, if it has any, to a draft built for it.
    pub(crate) fn with_threading(
        &self,
        composition: Option<&CompositionId>,
        draft: Draft,
    ) -> Draft {
        let threading = composition.and_then(|composition| {
            self.drafts
                .lock()
                .expect("drafts mutex poisoned")
                .open
                .get(composition)
                .and_then(|open| open.threading.clone())
        });
        match threading {
            Some(threading) => draft.in_reply_to(threading.in_reply_to, threading.references),
            None => draft,
        }
    }

    /// Records the key a save stored the composition under, with no digest.
    fn adopt_saved_key(&self, composition: &CompositionId, key: engine_api::ProviderKey) {
        if let Some(open) = self
            .drafts
            .lock()
            .expect("drafts mutex poisoned")
            .open
            .get_mut(composition)
        {
            open.key = Some(key);
        }
    }
}

/// Writes the files a composer can hold into `directory`, all or nothing.
///
/// An inline image belongs to the formatted body, which the composer does not open with
/// (`docs/drafts.md`, known gaps), so it is left out exactly as a resumed draft's is; the saved
/// draft still carries it until the composer's first save.
fn stage_draft_files(draft: &Draft, directory: &str) -> Result<Vec<StagedAttachment>, String> {
    let files: Vec<_> = draft
        .attachments
        .iter()
        .filter(|file| matches!(file.disposition, DraftAttachmentDisposition::Attachment))
        .collect();
    if files.is_empty() {
        return Ok(Vec::new());
    }
    std::fs::create_dir_all(directory).map_err(|err| err.to_string())?;
    let mut staged = Vec::with_capacity(files.len());
    for file in files {
        let seq = STAGE_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path = Path::new(directory).join(format!("{seq}-{}", safe_name(&file.file_name)));
        std::fs::write(&path, &file.content).map_err(|err| err.to_string())?;
        staged.push(StagedAttachment {
            path: path.to_string_lossy().into_owned(),
            file_name: file.file_name.clone(),
            media_type: file.media_type.clone(),
        });
    }
    log::info!(
        "composer: staged {} file(s) from a queued send",
        staged.len()
    );
    Ok(staged)
}

/// The file's name with anything that could leave the directory taken out. It comes from a
/// composer on this device, but the payload is stored, and a stored name is not trusted.
fn safe_name(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| {
            if matches!(c, '/' | '\\' | '\0') {
                '_'
            } else {
                c
            }
        })
        .collect();
    match cleaned.trim_matches('.') {
        "" => "attachment".to_owned(),
        _ => cleaned,
    }
}

/// A value no two compositions this process mints share.
fn mint() -> u64 {
    STAGE_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

fn join(addresses: &[engine_api::EmailAddress]) -> String {
    addresses
        .iter()
        .map(|address| address.email.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    use super::safe_name;

    #[test]
    fn a_stored_file_name_cannot_leave_the_staging_directory() {
        assert_eq!(safe_name("../../etc/passwd"), ".._.._etc_passwd");
        assert_eq!(safe_name("a\\b.pdf"), "a_b.pdf");
        assert_eq!(safe_name(".."), "attachment");
        assert_eq!(safe_name("report.pdf"), "report.pdf");
    }
}
