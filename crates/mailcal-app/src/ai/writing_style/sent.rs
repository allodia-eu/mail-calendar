//! Reading an account's sent mail for learning (`docs/ai.md` rule 14), on the device.
//!
//! Only the person's own words: every drafted reply is left out, whether it carries the drafted
//! keyword (which reaches every device where the provider keeps one) or is in this device's log
//! (rule 25). Bodies come through the engine's body read, which answers from the local cache for
//! what the body warm has reached and fetches the rest once, as opening it would; the text is a
//! message's own text part, or the text of its sanitised HTML, never the HTML.

use std::cmp::Reverse;

use engine_api::{AccountId, Message, Provider, UtcDateTime, find_calendar_part};
use mailcal_account::drafted_reply_keyword;
use mailcal_ai::corpus::SentMessage;

use super::range::admits;
use crate::{App, mail_ops::sent_keys, query::plain_text_of};

/// The most sent messages one learning run reads. The sampler keeps far fewer; this bounds the
/// time spent reading bodies on a mailbox with years of sent mail.
const MAX_LEARN_MESSAGES: usize = 1_500;

/// The seconds since the Unix epoch of `instant`.
fn unix_seconds(instant: UtcDateTime) -> Option<i64> {
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

/// The messages of `messages` to read: in one of the `sent` folders, inside `bounds` and not
/// `drafted`, newest first and capped; with the oldest sent message the device holds, whatever
/// the range.
pub(super) fn learnable(
    messages: Vec<Message>,
    sent: &[String],
    bounds: (Option<i64>, Option<i64>),
    drafted: impl Fn(&Message) -> bool,
) -> (Vec<Message>, Option<i64>) {
    let mut messages: Vec<Message> = messages
        .into_iter()
        .filter(|message| {
            message
                .mailboxes
                .iter()
                .any(|mailbox| sent.iter().any(|key| key == mailbox.key().as_str()))
        })
        .collect();
    let horizon = messages.iter().filter_map(sent_at).min();
    messages.retain(|message| admits(bounds, sent_at(message)) && !drafted(message));
    messages.sort_by_key(|message| Reverse(sent_at(message)));
    messages.truncate(MAX_LEARN_MESSAGES);
    (messages, horizon)
}

impl<P: Provider> App<P> {
    /// Whether `message` is a drafted reply: it carries the keyword, or this device logged it.
    pub(super) fn drafted(&self, message: &Message) -> bool {
        message.has_keyword(&drafted_reply_keyword())
            || message
                .envelope
                .message_id
                .iter()
                .any(|id| self.is_drafted_reply(id))
    }

    /// The account's sent mail inside `bounds` as the corpus builder takes it, with the oldest
    /// sent message the device holds; `None` when the account has no Sent folder.
    pub(super) async fn learnable_sent_mail(
        &self,
        account: &AccountId,
        bounds: (Option<i64>, Option<i64>),
    ) -> Option<(Vec<SentMessage>, Option<i64>)> {
        let mailboxes = self.engine.mailboxes(account).await.unwrap_or_default();
        let sent = sent_keys(&mailboxes);
        if sent.is_empty() {
            return None;
        }
        let messages = self.engine.messages(account).await.unwrap_or_default();
        let (messages, horizon) =
            learnable(messages, &sent, bounds, |message| self.drafted(message));
        let mut out = Vec::with_capacity(messages.len());
        for message in messages {
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
        Some((out, horizon))
    }

    /// A message's body as plain text, or `None` when it cannot be read.
    async fn plain_body(&self, account: &AccountId, message: &Message) -> Option<String> {
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
                .map(|html| plain_text_of(&engine_html::sanitize(html).into_html())),
        }
    }

    /// Whether the account's provider keeps the drafted keyword on a Sent copy, so every device
    /// can leave a drafted reply out; without it only this device's log can.
    pub(super) async fn keeps_drafted_keyword(&self, account: &AccountId) -> bool {
        self.account_handle(account).await.is_some_and(|handle| {
            handle.providers.first().is_some_and(|provider| {
                provider.connection_info().capabilities.sent_copy_keywords()
            })
        })
    }
}
