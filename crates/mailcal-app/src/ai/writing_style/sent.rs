//! Reading an account's sent mail for learning (`docs/ai.md` rule 14), on the device.
//!
//! Only the person's own words: every drafted reply is left out, as
//! [`App::is_drafted_message`] recognises it (rule 25). Bodies are read as
//! `App::message_text` reads them, the text a draft's context is read as too. The messages are
//! each Sent mailbox's span over the range, which also says how far back the device holds it.

use std::{cmp::Reverse, collections::HashSet};

use engine_api::{AccountId, Message, Provider, UtcDateTime, find_calendar_part};
use mailcal_ai::corpus::SentMessage;

use super::range::admits;
use crate::{App, mail_ops::sent_mailboxes};

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

/// The instant `seconds` after the Unix epoch, clamped to the instants a span can name.
fn instant(seconds: i64) -> UtcDateTime {
    let at = time::OffsetDateTime::from_unix_timestamp(seconds)
        .unwrap_or(time::OffsetDateTime::UNIX_EPOCH)
        .to_offset(time::UtcOffset::UTC);
    UtcDateTime::new(
        at.year().clamp(1970, 9999),
        u8::from(at.month()),
        at.day(),
        at.hour(),
        at.minute(),
        at.second(),
    )
    .unwrap_or_else(|_| UtcDateTime::new(1970, 1, 1, 0, 0, 0).expect("the epoch is an instant"))
}

/// `bounds` as the span a mailbox is read over, its end exclusive. An open end reaches as far as
/// a span can.
fn span_of(bounds: (Option<i64>, Option<i64>)) -> (UtcDateTime, UtcDateTime) {
    (
        instant(bounds.0.unwrap_or(0)),
        bounds.1.map_or_else(
            || UtcDateTime::new(9999, 12, 31, 23, 59, 59).expect("a real instant"),
            |until| instant(until.saturating_add(1)),
        ),
    )
}

fn sent_at(message: &Message) -> Option<i64> {
    message
        .sent_at
        .or(message.received_at)
        .and_then(unix_seconds)
}

/// The messages of `messages` to read: inside `bounds` by when they were sent and not `drafted`,
/// each once, newest first and capped.
pub(super) fn learnable(
    messages: Vec<Message>,
    bounds: (Option<i64>, Option<i64>),
    drafted: impl Fn(&Message) -> bool,
) -> Vec<Message> {
    let mut seen = HashSet::new();
    let mut messages: Vec<Message> = messages
        .into_iter()
        .filter(|message| seen.insert(message.id.clone()))
        .filter(|message| admits(bounds, sent_at(message)) && !drafted(message))
        .collect();
    messages.sort_by_key(|message| Reverse(sent_at(message)));
    messages.truncate(MAX_LEARN_MESSAGES);
    messages
}

impl<P: Provider> App<P> {
    /// The account's sent mail inside `bounds` as the corpus builder takes it, with when the
    /// oldest sent message the device holds was sent, whatever the range; `None` when the account
    /// has no Sent folder.
    pub(super) async fn learnable_sent_mail(
        &self,
        account: &AccountId,
        bounds: (Option<i64>, Option<i64>),
    ) -> Option<(Vec<SentMessage>, Option<i64>)> {
        let mailboxes = self.engine.mailboxes(account).await.unwrap_or_default();
        let sent: Vec<_> = sent_mailboxes(&mailboxes).collect();
        if sent.is_empty() {
            return None;
        }
        let (since, until) = span_of(bounds);
        let mut found = Vec::new();
        let mut horizon: Option<i64> = None;
        for mailbox in sent {
            let Ok(span) = self
                .engine
                .mail_in_mailbox_between(account, &mailbox.id, since, until, MAX_LEARN_MESSAGES)
                .await
            else {
                continue;
            };
            if let Some(oldest) = span.oldest.and_then(unix_seconds) {
                horizon = Some(horizon.map_or(oldest, |held| held.min(oldest)));
            }
            found.extend(span.messages);
        }
        let messages = learnable(found, bounds, |message| self.is_drafted_message(message));
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
