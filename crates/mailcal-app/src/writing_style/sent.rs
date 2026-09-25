//! Reading an account's sent mail as plain text, on the device.
//!
//! Bodies come through the engine's body read, which answers from the local cache for everything
//! the body warm has reached (`prefetch`); a message outside it is fetched once, as opening it
//! would. The text is the message's own `text/plain` part, which this app always writes, or the
//! text of its sanitised HTML otherwise; never the HTML.

use std::cmp::Reverse;

use engine_api::{AccountId, MailboxRole, Message, Provider, UtcDateTime, find_calendar_part};
use mailcal_ai::corpus::SentMessage;

use super::{WritingStyleError, learn::LearnRange};
use crate::{App, mail_ops::resolve_move_target, query::to_plain};

/// The most sent messages one learning run reads. The sampler keeps far fewer; this bounds the
/// time spent reading bodies on a mailbox with years of sent mail.
const MAX_LEARN_MESSAGES: usize = 1_500;

/// How many of the newest messages a draft looks through for what was last sent to the same
/// person.
const RECIPIENT_WINDOW: usize = 600;

/// The seconds since the Unix epoch of `instant`.
pub(super) fn unix_seconds(instant: UtcDateTime) -> Option<i64> {
    let month = time::Month::try_from(instant.month()).ok()?;
    let date = time::Date::from_calendar_date(instant.year(), month, instant.day()).ok()?;
    let time = time::Time::from_hms(instant.hour(), instant.minute(), instant.second()).ok()?;
    Some(date.with_time(time).assume_utc().unix_timestamp())
}

fn sent_at(message: &Message) -> Option<i64> {
    message
        .sent_at
        .or(message.received_at)
        .and_then(unix_seconds)
}

impl<P: Provider> App<P> {
    /// The account's sent mail within `range`, newest first and capped, as the corpus builder
    /// takes it; with the oldest sent message this device holds, whatever the range.
    pub(super) async fn sent_mail(
        &self,
        account: &AccountId,
        range: LearnRange,
    ) -> Result<(Vec<SentMessage>, Option<i64>), WritingStyleError> {
        let mailboxes = self.engine.mailboxes(account).await.unwrap_or_default();
        let sent = resolve_move_target(&mailboxes, &MailboxRole::Sent)
            .ok_or(WritingStyleError::NoSentFolder)?
            .id
            .clone();
        let mut messages: Vec<Message> = self
            .engine
            .messages(account)
            .await
            .unwrap_or_default()
            .into_iter()
            .filter(|message| message.mailboxes.contains(&sent))
            .collect();
        let horizon = messages.iter().filter_map(sent_at).min();
        messages.retain(|message| range.admits(sent_at(message)));
        messages.sort_by_key(|message| Reverse(sent_at(message)));
        messages.truncate(MAX_LEARN_MESSAGES);

        let mut out = Vec::with_capacity(messages.len());
        for message in messages {
            if self.sent_from_a_draft(&message) {
                continue;
            }
            let Some(body) = self.plain_body(account, &message).await else {
                continue;
            };
            out.push(SentMessage {
                key: message.id.as_str().to_owned(),
                sent_at: sent_at(&message),
                recipient: message
                    .envelope
                    .to
                    .first()
                    .map(|address| address.email.to_lowercase()),
                subject: message.envelope.subject.clone().unwrap_or_default(),
                calendar: message
                    .mime_structure
                    .as_ref()
                    .is_some_and(|root| find_calendar_part(root).is_some()),
                body,
            });
        }
        log::info!("ai: read {} sent message(s) on this device", out.len());
        Ok((out, horizon))
    }

    /// The bodies of the last `count` messages the account sent to `address`, newest first.
    pub(super) async fn sent_to(
        &self,
        account: &AccountId,
        address: &str,
        count: usize,
    ) -> Vec<String> {
        let mailboxes = self.engine.mailboxes(account).await.unwrap_or_default();
        let Some(sent) = resolve_move_target(&mailboxes, &MailboxRole::Sent) else {
            return Vec::new();
        };
        let keys: Vec<_> = self
            .engine
            .mail_window(std::slice::from_ref(account), RECIPIENT_WINDOW)
            .await
            .unwrap_or_default()
            .into_iter()
            .filter(|row| row.mailboxes.contains(&sent.id))
            .map(|row| row.mail.key)
            .collect();
        let mut messages: Vec<Message> = self
            .engine
            .messages_by_keys(account, &keys)
            .await
            .unwrap_or_default()
            .into_iter()
            .filter(|message| {
                !self.sent_from_a_draft(message)
                    && message
                        .envelope
                        .to
                        .iter()
                        .chain(&message.envelope.cc)
                        .any(|recipient| recipient.email.eq_ignore_ascii_case(address))
            })
            .collect();
        messages.sort_by_key(|message| Reverse(sent_at(message)));
        let mut bodies = Vec::new();
        for message in messages.into_iter().take(count) {
            if let Some(body) = self.plain_body(account, &message).await {
                bodies.push(body);
            }
        }
        bodies
    }

    /// Whether `message` was sent from an AI draft: never learned from, and never shown to a
    /// draft as the person's own words.
    fn sent_from_a_draft(&self, message: &Message) -> bool {
        message
            .envelope
            .message_id
            .first()
            .is_some_and(|id| self.writing_style.observed.is_assisted(id.as_str()))
    }

    /// A message's body as plain text, or `None` when it cannot be read.
    pub(super) async fn plain_body(
        &self,
        account: &AccountId,
        message: &Message,
    ) -> Option<String> {
        let handle = self.account_handle(account).await?;
        let provider = handle.providers.first()?;
        let body = self
            .engine
            .message_body(provider, account, message)
            .await
            .ok()?;
        match body.plain() {
            Some(plain) if !plain.trim().is_empty() => Some(plain.to_owned()),
            _ => body
                .html()
                .map(|html| to_plain(&crate::html::sanitize(html).html)),
        }
    }
}
