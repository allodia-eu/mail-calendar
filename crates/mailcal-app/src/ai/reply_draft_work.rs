//! Making a draft (`docs/ai.md` rules 1, 3 and 26): the gate is asked before anything is read,
//! then the message answered, what the person last sent its author and the signature the composer
//! adds are read on the device, and one request goes out. A drafted reply is recognised by the
//! keyword on its Sent copy, read from the store's listing, or by this device's log.

use std::cmp::Reverse;

use engine_api::{AccountId, MailboxRole, Message, MessageIdHeader, Provider};
use mailcal_account::drafted_reply_keyword;
use mailcal_ai::{
    DraftRequest, Exemplars, LanguageCode, StyleGuide, ThreadMessage, corpus, draft_reply,
};
use mailcal_viewmodel::SignatureSlotKind;

use super::{AiFailure, AiRun, reply_draft_flow::Made};
use crate::{App, CompositionId, mail_ops::resolve_move_target, reference::MessageRef};

/// How many of the person's last messages to the same address a draft reads.
const RECIPIENT_MESSAGES: usize = 2;

/// How many of the newest messages are looked through for them.
const RECIPIENT_WINDOW: usize = 600;

/// What a draft is made from, besides the style.
struct ReplyContext {
    thread: Vec<ThreadMessage>,
    recipient_messages: Vec<String>,
    signature: Option<String>,
}

impl<P: Provider> App<P> {
    /// Makes the draft for `composition`, sent from `sender`. `None` when it was stopped or its
    /// composition has gone, which keeps nothing.
    pub(super) fn make_reply_draft(
        &self,
        composition: &CompositionId,
        sender: &AccountId,
        instruction: &str,
        language: &str,
        run: &AiRun<'_>,
    ) -> Option<Result<Made, AiFailure>> {
        if run.cancelled() {
            return None;
        }
        // The gate, before anything is read (rule 1).
        let backend = match self.ai_backend(run) {
            Ok(backend) => backend,
            Err(failure) => return Some(Err(failure)),
        };
        let (guide, exemplars) = match self.drafting_style(sender) {
            Ok(style) => style,
            Err(failure) => return Some(Err(failure)),
        };
        let (opened_on, answering, in_reply_to) = self.reply_of(composition, |reply| {
            (
                reply.account.clone(),
                reply.answering.cloned(),
                reply.in_reply_to.cloned(),
            )
        })?;
        let context = run.wait(self.reply_context(sender, &opened_on, answering, in_reply_to));
        let Some(context) = context else {
            return Some(Err(AiFailure::MessageGone));
        };
        if run.cancelled() {
            return None;
        }
        let ui_language = LanguageCode::new(language)
            .or_else(|| LanguageCode::new("en"))
            .expect("a two-letter code");
        let instruction = Some(instruction.trim()).filter(|text| !text.is_empty());
        let request = DraftRequest {
            thread: &context.thread,
            guide: &guide,
            exemplars: &exemplars,
            recipient_messages: &context.recipient_messages,
            intent: instruction,
            language: None,
            signature: context.signature.as_deref(),
            ui_language: &ui_language,
        };
        match draft_reply(&request, &backend) {
            Ok(draft) => Some(Ok(Made {
                text: draft.text,
                summary: draft.summary,
                tasks: draft.tasks,
            })),
            Err(error) => AiFailure::of_request(&error, backend.destination()).map(Err),
        }
    }

    /// The description and passages of the style `account` drafts in.
    fn drafting_style(&self, account: &AccountId) -> Result<(StyleGuide, Exemplars), AiFailure> {
        let id = self
            .writing_style_slot(account.as_str())
            .ok_or(AiFailure::NoStyleForAccount)?;
        let style = self
            .writing_style(&id)
            .ok_or(AiFailure::NoStyleForAccount)?;
        let guide = serde_json::from_str(&style.guide_json).map_err(|_| {
            log::warn!("ai: the style's description could not be read");
            AiFailure::NoStyleForAccount
        })?;
        let exemplars = if style.exemplars_json.trim().is_empty() {
            Exemplars::new()
        } else {
            serde_json::from_str(&style.exemplars_json).unwrap_or_else(|_| Exemplars::new())
        };
        Ok((guide, exemplars))
    }

    /// The message answered with what the person last sent its author, or `None` when the
    /// message is no longer on this device.
    async fn reply_context(
        &self,
        sender: &AccountId,
        opened_on: &AccountId,
        answering: Option<MessageRef>,
        in_reply_to: Option<MessageIdHeader>,
    ) -> Option<ReplyContext> {
        let (account, original) = match (answering, in_reply_to) {
            (Some(message), _) => {
                let original = self.find_message_in(&message).await?;
                (message.account, original)
            }
            (None, Some(parent)) => {
                let original = self
                    .engine
                    .messages(opened_on)
                    .await
                    .ok()?
                    .into_iter()
                    .find(|message| message.envelope.message_id.contains(&parent))?;
                (opened_on.clone(), original)
            }
            (None, None) => return None,
        };
        let body = self.plain_text_of(&account, &original).await?;
        let author = original.envelope.from.first();
        let thread = vec![ThreadMessage {
            from: author.map_or_else(String::new, crate::reading::format_address),
            date: original
                .sent_at
                .or(original.received_at)
                .map_or_else(String::new, crate::query::format_instant),
            body,
        }];
        let signatures: Vec<String> = [
            SignatureSlotKind::NewMessage,
            SignatureSlotKind::ReplyForward,
        ]
        .into_iter()
        .filter_map(|slot| self.resolve_signature(sender.as_str(), slot))
        .map(|signature| signature.body_plain)
        .collect();
        let recipient_messages = match author {
            Some(address) => self
                .sent_to(sender, &address.email, RECIPIENT_MESSAGES)
                .await
                .iter()
                .map(|body| corpus::own_text(body, &signatures))
                .filter(|text| !text.trim().is_empty())
                .collect(),
            None => Vec::new(),
        };
        let signature = self
            .resolve_signature(sender.as_str(), SignatureSlotKind::ReplyForward)
            .map(|signature| signature.body_plain)
            .filter(|plain| !plain.trim().is_empty());
        Some(ReplyContext {
            thread,
            recipient_messages,
            signature,
        })
    }

    /// The bodies of the last `count` messages `account` sent to `address`, newest first,
    /// leaving out every drafted reply (rule 25).
    async fn sent_to(&self, account: &AccountId, address: &str, count: usize) -> Vec<String> {
        let mailboxes = self.engine.mailboxes(account).await.unwrap_or_default();
        let Some(sent) = resolve_move_target(&mailboxes, &MailboxRole::Sent) else {
            return Vec::new();
        };
        let keyword = drafted_reply_keyword();
        let keys: Vec<_> = self
            .engine
            .mail_window(std::slice::from_ref(account), RECIPIENT_WINDOW)
            .await
            .unwrap_or_default()
            .into_iter()
            .filter(|row| row.mailboxes.contains(&sent.id) && !row.keywords.contains(&keyword))
            .map(|row| row.mail.key)
            .collect();
        let mut messages: Vec<Message> = self
            .engine
            .messages_by_keys(account, &keys)
            .await
            .unwrap_or_default()
            .into_iter()
            .filter(|message| {
                !self.is_drafted_message(message)
                    && message
                        .envelope
                        .to
                        .iter()
                        .chain(&message.envelope.cc)
                        .any(|recipient| recipient.email.eq_ignore_ascii_case(address))
            })
            .collect();
        messages.sort_by_key(|message| Reverse(message.sent_at.or(message.received_at)));
        let mut bodies = Vec::new();
        for message in messages.into_iter().take(count) {
            if let Some(body) = self.plain_text_of(account, &message).await {
                bodies.push(body);
            }
        }
        bodies
    }

    /// A message's body as text: its `text/plain` part, or the text of its HTML.
    async fn plain_text_of(&self, account: &AccountId, message: &Message) -> Option<String> {
        let handle = self.account_handle(account).await?;
        let provider = handle.providers.first()?;
        let body = self
            .engine
            .message_body(provider, account, message)
            .await
            .ok()?;
        match body.plain() {
            Some(plain) if !plain.trim().is_empty() => Some(plain.to_owned()),
            _ => body.html().map(crate::query::to_plain),
        }
    }

    /// Whether `message`, as the store lists it, is a drafted reply: it carries the keyword, which
    /// reaches every device where the provider keeps one, or this device's log names it. Never
    /// learned from and never shown to a draft as the person's own words (rule 25).
    #[must_use]
    pub fn is_drafted_message(&self, message: &Message) -> bool {
        message.has_keyword(&drafted_reply_keyword())
            || message
                .envelope
                .message_id
                .iter()
                .any(|id| self.is_drafted_reply(id))
    }
}
