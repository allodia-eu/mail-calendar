//! Comparing drafts, in a debug build only (`docs/ai.md`, "Training mode"): messages drafted
//! through a backend the caller builds for each model, under instructions the caller names, and
//! the record of each draft or of why none came back.
//!
//! Nothing here reaches a composer. No draft is issued, so none is remembered for feedback or for
//! the observations log, and no balance is recorded. The backend is a `GatedBackend` like any
//! other, so the gate and the endpoint's declaration apply to every request.

use std::{cmp::Reverse, collections::HashSet, time::Instant};

use engine_api::{MailListRow, MailboxRole, Provider};
use mailcal_ai::{DraftRecord, GatedBackend};

use super::{ReplyDraftRequest, WritingStyleError, draft::as_prompted};
use crate::{App, MessageRef, mail_ops::resolve_move_target};

/// How many of each account's newest messages are looked through for answered ones.
const ANSWERED_WINDOW: usize = 1_000;

impl<P: Provider> App<P> {
    /// Drafts a reply to `request.message` through `backend`, under the named instructions of
    /// `variant` or the default ones. Answers the draft's record with what it said, or, when none
    /// came back, a record without content, its schema version zero, and why.
    pub async fn training_draft(
        &self,
        request: &ReplyDraftRequest,
        backend: &GatedBackend,
        variant: Option<(&str, &str)>,
    ) -> (DraftRecord, Option<WritingStyleError>) {
        let (name, instructions) = variant.unzip();
        let started = Instant::now();
        match self.make_draft(request, backend, instructions).await {
            Ok(made) => (made.record(backend.model_label(), name), None),
            Err(error) => (
                DraftRecord {
                    model: backend.model_label().to_owned(),
                    variant: name.map(str::to_owned),
                    schema_version: 0,
                    language: String::new(),
                    elapsed_ms: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
                    usage: None,
                    content: None,
                },
                Some(error),
            ),
        }
    }

    /// The body of the message a comparison answers, as the prompt carried it, for its export.
    pub async fn training_message(&self, message: &MessageRef) -> Option<String> {
        let original = self.find_message_in(message).await?;
        let body = self.plain_body(&message.account, &original).await?;
        Some(as_prompted(&body))
    }

    /// The list rows of the newest `count` Inbox messages the person answered, across the
    /// accounts that draft in a style: a message in the account's Sent folder, among its newest,
    /// names each in `In-Reply-To`. Read from what the device holds; nothing is fetched.
    pub async fn training_answered(&self, count: usize) -> Vec<MailListRow> {
        let mut found = Vec::new();
        for account in self.account_ids().await {
            if self.writing_style.assigned(account.as_str()).is_none() {
                continue;
            }
            let mailboxes = self.engine.mailboxes(&account).await.unwrap_or_default();
            let (Some(inbox), Some(sent)) = (
                resolve_move_target(&mailboxes, &MailboxRole::Inbox),
                resolve_move_target(&mailboxes, &MailboxRole::Sent),
            ) else {
                continue;
            };
            let rows = self
                .engine
                .mail_window(std::slice::from_ref(&account), ANSWERED_WINDOW)
                .await
                .unwrap_or_default();
            let sent_keys: Vec<_> = rows
                .iter()
                .filter(|row| row.mailboxes.contains(&sent.id))
                .map(|row| row.mail.key.clone())
                .collect();
            let replied_to: HashSet<String> = self
                .engine
                .messages_by_keys(&account, &sent_keys)
                .await
                .unwrap_or_default()
                .iter()
                .flat_map(|message| &message.envelope.in_reply_to)
                .map(|id| id.as_str().to_owned())
                .collect();
            found.extend(rows.into_iter().filter(|row| {
                row.mailboxes.contains(&inbox.id)
                    && row
                        .mail
                        .message_id
                        .as_ref()
                        .is_some_and(|id| replied_to.contains(id.as_str()))
            }));
        }
        found.sort_by_key(|row| Reverse(row.mail.date_utc));
        found.truncate(count);
        found
    }
}
