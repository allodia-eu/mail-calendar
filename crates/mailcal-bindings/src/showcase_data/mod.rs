//! The structured content of the [`crate::showcase`] screenshot dataset: two accounts'
//! folders and messages, plus a calendar of events, all dated relative to a *pinned* now
//! (`seeded_now` — today's date at a fixed wall clock) so the screenshots always look current
//! without changing every time they are retaken. Bodies live in [`crate::showcase_bodies`].
//!
//! The dataset exists in **each language the app ships** — a store listing needs a screenshot
//! per locale, and half-translated chrome over English mail reads as broken. The seeds
//! themselves live in the per-locale submodules, one per catalog locale; this module holds the
//! locale dispatch and the mail builders they share (the calendar builders live in
//! `showcase_data::calendar`), and the [`ShowcaseLocale`] switch between them. That does **not**
//! make the core locale-aware: the seed is *sample provider data* (mail arrives in whatever
//! language its sender wrote), so picking a fixture is not the runtime locale facility the client
//! owns.
//!
//! Everything here is fictional. Addresses use reserved documentation domains
//! (`example.com`, `example.org`, and the `.example` TLD) and made-up company names; no
//! real person, mailbox, or provider brand is referenced.

use std::collections::HashMap;

// Pulled into this module's namespace so the per-locale seeds keep importing every builder
// from one place (`use super::{…}`), whichever file it actually lives in. A plain (private)
// import rather than a re-export on purpose: it is visible to this module's descendants —
// which is exactly the seeds — without widening the calendar builders' reach beyond
// `showcase_data`.
use calendar::{
    Cal, CalendarNames, FRI, MON, SAT, SUN, THU, TUE, WED, all_day_event, event, showcase_calendar,
    zoned_wd,
};
// Same reasoning as the calendar builders above: a private import, so this module's
// descendants — the per-locale seeds — keep writing `super::ago` wherever it happens to live.
use clock::ago;
pub(crate) use clock::seeded_now;
#[cfg(any(debug_assertions, feature = "dev-harness"))]
use engine_core::ids::{EventId, Uid};
use engine_core::{
    calendar::{Calendar, Event},
    ids::{MailboxId, MessageId, MessageIdHeader},
    mail::{EmailAddress, Keyword, Mailbox, MailboxRole, Message, SystemKeyword},
    membership::Memberships,
    time::UtcDateTime,
};
use time::OffsetDateTime;

use crate::showcase_bodies::body;

mod calendar;
mod clock;
mod de;
mod en;
mod es;
mod fr;
mod invitation;
mod it;
mod nl;
mod pt;

// The locale seeds address the invitation's organiser by name in the message header; taking it
// from the roster rather than retyping it is what keeps `From:` and the iTIP `ORGANIZER` the
// same person in all seven languages.
use invitation::organizer as invite_organizer;
pub(crate) use invitation::{InviteText, MESSAGE_KEY as INVITE_MESSAGE_KEY};

/// The language a showcase (screenshot) app is seeded in — its sample mail, folder names,
/// and calendar. A host picks it from the UI language it is about to render, so the mail in
/// a store screenshot reads in the same language as the chrome around it.
///
/// One variant per locale in the shared message catalog (`messages/<locale>.json`); keep the
/// two in step, or a language ships chrome with no sample mail to sit under it.
#[derive(uniffi::Enum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShowcaseLocale {
    /// English sample content.
    En,
    /// Dutch sample content.
    Nl,
    /// German sample content.
    De,
    /// French sample content.
    Fr,
    /// Spanish sample content.
    Es,
    /// Italian sample content.
    It,
    /// Portuguese sample content.
    Pt,
}

/// The meeting invitation's language-dependent text in `locale` — its title, room and notes.
///
/// One dispatch for both places that need it: the calendar hold built into
/// [`primary_calendar`], and the iTIP payload the mail carries
/// ([`crate::showcase_bodies::body`]). Reading it twice from one source is what keeps the card's
/// title and the block on the grid the same words.
pub(crate) fn invite_text(locale: ShowcaseLocale) -> InviteText {
    match locale {
        ShowcaseLocale::En => en::invite_text(),
        ShowcaseLocale::Nl => nl::invite_text(),
        ShowcaseLocale::De => de::invite_text(),
        ShowcaseLocale::Fr => fr::invite_text(),
        ShowcaseLocale::Es => es::invite_text(),
        ShowcaseLocale::It => it::invite_text(),
        ShowcaseLocale::Pt => pt::invite_text(),
    }
}

/// The iTIP `REQUEST` document the showcase invitation mail carries, in `locale`.
pub(crate) fn invite_ics(locale: ShowcaseLocale, now: OffsetDateTime) -> String {
    invitation::ics(&invite_text(locale), now)
}

/// The showcase locale to seed for a UI language code (`"de"` → [`ShowcaseLocale::De`]), so a
/// screenshot's sample mail always reads in the language of the chrome around it.
///
/// Every client resolves its chrome language to a bare code and calls this, rather than each
/// keeping its own switch: one mapping, so the three clients cannot drift apart. A code we do
/// not ship falls back to [`ShowcaseLocale::En`] — the catalog's base locale, and the same
/// fallback the generated `L10n` makes, so chrome and sample mail never disagree.
#[uniffi::export]
#[must_use]
pub fn showcase_locale_for_language(code: String) -> ShowcaseLocale {
    match code.as_str() {
        "nl" => ShowcaseLocale::Nl,
        "de" => ShowcaseLocale::De,
        "fr" => ShowcaseLocale::Fr,
        "es" => ShowcaseLocale::Es,
        "it" => ShowcaseLocale::It,
        "pt" => ShowcaseLocale::Pt,
        _ => ShowcaseLocale::En,
    }
}

/// The account that owns the message a showcase reply screenshot replies to. Both locales seed
/// the primary account under this identity, so it doubles as the account id.
const REPLY_ACCOUNT: &str = "eva.jansen@example.com";

/// The message a showcase reply screenshot replies to: Northwind Legal's "please sign" request,
/// which sits flagged in the primary Inbox in both locales. Deliberately a *standalone* message
/// rather than one of the threaded launch messages — those already carry Eva's reply and Tom's
/// answer, so a fresh reply to them would read as a non-sequitur in the screenshot.
const REPLY_MESSAGE_KEY: &str = "p-contract";

/// Which showcase message a screenshot run should reply to, and the sample text to open the
/// composer pre-filled with — so the "replying to a mail" screenshot shows a written reply
/// rather than an empty body, in the same language as the chrome around it.
///
/// The text is *sample content*, like the seeded mail itself: it is not the runtime locale
/// facility the client owns (see this module's header).
#[derive(uniffi::Record, Clone, Debug, PartialEq, Eq)]
pub struct ShowcaseReply {
    /// The id of the account holding the message (its owner address).
    pub account: String,
    /// The replied-to message's provider key, as it appears on a `FlatRow`.
    pub message_key: String,
    /// The plain-text reply body to seed the composer with, above the quoted original.
    pub text: String,
}

/// The [`ShowcaseReply`] for `locale`. A host in showcase mode opens `message_key`, begins a
/// reply, and seeds the composer with `text`.
#[uniffi::export]
pub fn showcase_reply(locale: ShowcaseLocale) -> ShowcaseReply {
    let text = match locale {
        ShowcaseLocale::En => en::reply_text(),
        ShowcaseLocale::Nl => nl::reply_text(),
        ShowcaseLocale::De => de::reply_text(),
        ShowcaseLocale::Fr => fr::reply_text(),
        ShowcaseLocale::Es => es::reply_text(),
        ShowcaseLocale::It => it::reply_text(),
        ShowcaseLocale::Pt => pt::reply_text(),
    };
    ShowcaseReply {
        account: REPLY_ACCOUNT.to_owned(),
        message_key: REPLY_MESSAGE_KEY.to_owned(),
        text: text.to_owned(),
    }
}

/// Which showcase message a screenshot run should open to show the meeting-invitation card —
/// the mail-and-calendar surface, where a message from the inbox offers Accept / Maybe / Decline
/// over a preview of the day it would land on.
///
/// The twin of [`ShowcaseReply`], and for the same reason: the target is one fact, held in the
/// core, so all three clients open the *same* message and a reworded seed cannot leave one of
/// them opening nothing.
#[derive(uniffi::Record, Clone, Debug, PartialEq, Eq)]
pub struct ShowcaseInvitation {
    /// The id of the account holding the message (its owner address).
    pub account: String,
    /// The invitation message's provider key, as it appears on a `FlatRow`.
    pub message_key: String,
}

/// The [`ShowcaseInvitation`] a host opens for the invitation screenshot.
///
/// Takes no locale: the message key and the account are identical in every seed (only the
/// meeting's words are translated), and a parameter that can only ever be ignored would imply a
/// choice that does not exist.
#[uniffi::export]
#[must_use]
pub fn showcase_invitation() -> ShowcaseInvitation {
    ShowcaseInvitation {
        account: REPLY_ACCOUNT.to_owned(),
        message_key: invitation::MESSAGE_KEY.to_owned(),
    }
}

/// One seeded account's data: its owner address (also its id and switcher label), its
/// folders, its messages, and the tailored raw source for the messages that have one.
pub(crate) struct AccountSeed {
    /// The owner's email address.
    pub(crate) identity: String,
    /// The account's folders (Inbox, Sent, …), each with a role for sidebar ordering.
    pub(crate) mailboxes: Vec<Mailbox>,
    /// The account's messages, spread across its folders.
    pub(crate) messages: Vec<Message>,
    /// Provider key → tailored raw MIME source (only for messages that have one).
    pub(crate) bodies: HashMap<String, Vec<u8>>,
}

/// One seeded showcase signature: what the library calls it, and the two bodies a composer
/// needs (the HTML it seeds into the editor, and the plain-text rendering that rides alongside
/// it into the text/plain part).
///
/// The showcase library exists so a store screenshot shows the feature rather than its empty
/// state: with no signature the composer's Signature control does not appear at all, and the
/// Settings category renders "You haven't written a signature yet" — accurate, but it advertises
/// nothing (`docs/store-listing.md`). Deliberately text-only: an embedded logo would mean a
/// binary asset in this dataset, and the point here is the surface, not the image pipeline.
pub(crate) struct SignatureSeed {
    /// The library name, as the picker and the Settings list show it.
    pub(crate) name: &'static str,
    /// The HTML body, as the signature editor would have produced it.
    pub(crate) body_html: &'static str,
    /// Its plain-text rendering.
    pub(crate) body_plain: &'static str,
}

/// The showcase library: one signature per seeded account, each assigned to **both** of that
/// account's slots — so a new message and a reply both open with one, which is what the
/// screenshots need to show.
pub(crate) struct ShowcaseSignatures {
    /// The primary account's signature (the fuller block; it is the account the reply
    /// screenshot composes from).
    pub(crate) primary: SignatureSeed,
    /// The secondary account's.
    pub(crate) secondary: SignatureSeed,
}

/// The seeded signature library for `locale`.
pub(crate) fn signatures(locale: ShowcaseLocale) -> ShowcaseSignatures {
    match locale {
        ShowcaseLocale::En => en::signatures(),
        ShowcaseLocale::Nl => nl::signatures(),
        ShowcaseLocale::De => de::signatures(),
        ShowcaseLocale::Fr => fr::signatures(),
        ShowcaseLocale::Es => es::signatures(),
        ShowcaseLocale::It => it::signatures(),
        ShowcaseLocale::Pt => pt::signatures(),
    }
}

/// The primary account: a full mailbox (Inbox + Sent + Archive + Drafts) with a threaded
/// conversation, a flagged item, an attachment, and a remote-image newsletter.
pub(crate) fn primary(locale: ShowcaseLocale, now: OffsetDateTime) -> AccountSeed {
    match locale {
        ShowcaseLocale::En => en::primary(now),
        ShowcaseLocale::Nl => nl::primary(now),
        ShowcaseLocale::De => de::primary(now),
        ShowcaseLocale::Fr => fr::primary(now),
        ShowcaseLocale::Es => es::primary(now),
        ShowcaseLocale::It => it::primary(now),
        ShowcaseLocale::Pt => pt::primary(now),
    }
}

/// The secondary account: a lighter Inbox, so the unified inbox and account switcher show
/// two accounts.
pub(crate) fn secondary(locale: ShowcaseLocale, now: OffsetDateTime) -> AccountSeed {
    match locale {
        ShowcaseLocale::En => en::secondary(now),
        ShowcaseLocale::Nl => nl::secondary(now),
        ShowcaseLocale::De => de::secondary(now),
        ShowcaseLocale::Fr => fr::secondary(now),
        ShowcaseLocale::Es => es::secondary(now),
        ShowcaseLocale::It => it::secondary(now),
        ShowcaseLocale::Pt => pt::secondary(now),
    }
}

/// The primary account's calendars and their events. Three calendars — Work, Personal, and
/// Family — each in its own palette hue, with events spread across the **whole current week**
/// and across the day, so a screenshot shows a real, colourful work-and-private-life calendar
/// rather than a wall of one colour on an empty week.
pub(crate) fn primary_calendar(
    locale: ShowcaseLocale,
    now: OffsetDateTime,
) -> (Vec<Calendar>, Vec<Event>) {
    let (names, mut events) = match locale {
        ShowcaseLocale::En => en::calendar(now),
        ShowcaseLocale::Nl => nl::calendar(now),
        ShowcaseLocale::De => de::calendar(now),
        ShowcaseLocale::Fr => fr::calendar(now),
        ShowcaseLocale::Es => es::calendar(now),
        ShowcaseLocale::It => it::calendar(now),
        ShowcaseLocale::Pt => pt::calendar(now),
    };
    // The unanswered hold for the seeded meeting invitation, appended here rather than in each
    // locale's own list: it is the *same* meeting in every language, and its `UID`, start and
    // roster have to match the mail's iTIP payload exactly (`invitation`). Seven copies of that
    // agreement is seven chances to break it.
    events.push(invitation::hold(&invite_text(locale), now));
    #[cfg(any(debug_assertions, feature = "dev-harness"))]
    densify_for_performance(&mut events);
    let calendars = vec![
        // Work is the default calendar for new events; the others are its siblings.
        showcase_calendar(Cal::Work, names.work, true),
        showcase_calendar(Cal::Personal, names.personal, false),
        showcase_calendar(Cal::Family, names.family, false),
    ];
    (calendars, events)
}

/// Expands the fictional showcase diary to the requested visible-event load for a release-profile
/// renderer measurement. The feature gate keeps the environment hook out of shipped builds.
#[cfg(any(debug_assertions, feature = "dev-harness"))]
fn densify_for_performance(events: &mut Vec<Event>) {
    let Some(target) = std::env::var("MAILCAL_CALENDAR_PERF_EVENTS")
        .ok()
        .and_then(|raw| raw.parse::<usize>().ok())
        .map(|target| target.min(10_000))
        .filter(|target| *target > events.len())
    else {
        return;
    };
    let templates = events.clone();
    while events.len() < target {
        let index = events.len();
        let mut event = templates[index % templates.len()].clone();
        let key = format!("performance-event-{index}");
        event.id = EventId::try_from(key.as_str()).expect("fixture event id");
        event.uid = Uid::new(format!("{key}@allodia.local")).expect("fixture event uid");
        events.push(event);
    }
}

/// Bundles a built account's parts, collecting the tailored bodies for its messages.
///
/// `now` reaches the bodies because one of them — the meeting invitation — carries an iTIP
/// payload dated to the current week, and it has to name the same instant the calendar hold does.
fn seed(
    locale: ShowcaseLocale,
    identity: &str,
    mut mailboxes: Vec<Mailbox>,
    messages: Vec<Message>,
    now: OffsetDateTime,
) -> AccountSeed {
    let bodies = messages
        .iter()
        .filter_map(|m| {
            let key = m.id.key().as_str().to_owned();
            body(locale, &key, now).map(|mime| (key, mime))
        })
        .collect();
    count_unread(&mut mailboxes, &messages);
    AccountSeed {
        identity: identity.to_owned(),
        mailboxes,
        messages,
        bodies,
    }
}

/// Gives each seeded folder the unread count its own seeded messages imply.
///
/// Derived rather than written into every locale's seed: a hand-written number would be one
/// more thing seven locales have to keep in step, and a screenshot showing "3" beside a folder
/// holding one bold row is worse than showing nothing. A real provider reports the server's
/// count, which here *is* the seeded mailbox — nothing is outside the window.
fn count_unread(mailboxes: &mut [Mailbox], messages: &[Message]) {
    for mailbox in mailboxes {
        let unread = messages
            .iter()
            .filter(|message| message.mailboxes.contains(&mailbox.id) && message.is_unread())
            .count();
        mailbox.unread_count = Some(u32::try_from(unread).unwrap_or(u32::MAX));
    }
}

fn mailbox(id: &str, name: &str, role: MailboxRole) -> Mailbox {
    let mut mailbox = Mailbox::new(MailboxId::try_from(id).expect("valid mailbox id"), name);
    mailbox.role = Some(role);
    mailbox
}

/// A small fluent builder over [`Message`], so the seeds read as data.
struct MsgBuilder {
    message: Message,
}

fn msg(key: &str, mailbox: &str) -> MsgBuilder {
    MsgBuilder {
        message: Message::new(
            MessageId::try_from(key).expect("valid message id"),
            Memberships::of_one(MailboxId::try_from(mailbox).expect("valid mailbox id")),
        ),
    }
}

impl MsgBuilder {
    fn subject(mut self, subject: &str) -> Self {
        self.message.envelope.subject = Some(subject.to_owned());
        self
    }

    fn from(mut self, name: &str, email: &str) -> Self {
        self.message.envelope.from = vec![EmailAddress::named(name, email)];
        self
    }

    fn to(mut self, name: &str, email: &str) -> Self {
        self.message.envelope.to = vec![EmailAddress::named(name, email)];
        self
    }

    fn preview(mut self, preview: &str) -> Self {
        self.message.preview = Some(preview.to_owned());
        self
    }

    fn at(mut self, when: UtcDateTime) -> Self {
        self.message.received_at = Some(when);
        self
    }

    fn id(mut self, message_id: &str) -> Self {
        self.message.envelope.message_id =
            vec![MessageIdHeader::new(message_id).expect("valid message-id")];
        self
    }

    /// Links this message into a conversation: an `In-Reply-To` root and the `References` chain.
    fn thread(mut self, in_reply_to: &str, references: &[&str]) -> Self {
        self.message.envelope.in_reply_to =
            vec![MessageIdHeader::new(in_reply_to).expect("valid message-id")];
        self.message.envelope.references = references
            .iter()
            .map(|id| MessageIdHeader::new(*id).expect("valid message-id"))
            .collect();
        self
    }

    fn seen(mut self) -> Self {
        self.message
            .keywords
            .insert(Keyword::system(SystemKeyword::Seen));
        self
    }

    fn flagged(mut self) -> Self {
        self.message
            .keywords
            .insert(Keyword::system(SystemKeyword::Flagged));
        self
    }

    fn draft(mut self) -> Self {
        self.message
            .keywords
            .insert(Keyword::system(SystemKeyword::Draft));
        self
    }

    fn attachment(mut self) -> Self {
        self.message.has_attachment = true;
        self
    }

    fn done(self) -> Message {
        self.message
    }
}
