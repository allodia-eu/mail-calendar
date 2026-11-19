//! Reading an account's sent mail for learning (`docs/ai.md` rule 14), on the device.
//!
//! Only the person's own words: every drafted reply is left out, as
//! [`App::is_drafted_message`] recognises it (rule 25). Bodies are read as
//! `App::message_text` reads them, the text a draft's context is read as too.

use std::cmp::Reverse;

use engine_api::{AccountId, Message, Provider, UtcDateTime, find_calendar_part};
use mailcal_ai::corpus::SentMessage;

use super::range::admits;
use crate::{App, mail_ops::sent_keys};

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
        let (messages, horizon) = learnable(messages, &sent, bounds, |message| {
            self.is_drafted_message(message)
        });
        let mut out = Vec::with_capacity(messages.len());
        for message in messages {
            let Some(body) = self.message_text(account, &message).await else {
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
