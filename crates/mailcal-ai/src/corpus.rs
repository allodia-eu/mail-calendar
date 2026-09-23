//! The corpus: from an account's sent mail to the sample a style is learned from, all on the
//! device.
//!
//! In order: cut every message to the words its author wrote (`strip`); drop what says nothing
//! about how they write (calendar answers, automatic replies, near-duplicates, anything under
//! [`MIN_WORDS`]); name each message's language; sample each language under a token budget
//! (`sample`). The [`CorpusReport`] says what was found at each step, so the person sees what
//! would be sent, and roughly how much of it, before anything is.

use std::{
    collections::{BTreeMap, HashSet},
    fmt,
};

use crate::language;

mod sample;
mod strip;

pub(crate) use sample::estimate_tokens;

/// The fewest words of their own a message needs to say anything about how someone writes.
pub const MIN_WORDS: usize = 40;

/// Roughly how many tokens of mail one language's sample may hold.
pub const LANGUAGE_BUDGET_TOKENS: usize = 120_000;

/// How many leading characters decide whether two messages are the same message sent twice.
const DUPLICATE_PREFIX_CHARS: usize = 200;

/// Subject openings of automatic replies, in the languages mail servers write them in.
const AUTOMATIC_SUBJECTS: [&str; 12] = [
    "automatic reply",
    "auto:",
    "autoreply",
    "out of office",
    "automatisch antwoord",
    "afwezig",
    "automatische antwort",
    "abwesenheitsnotiz",
    "réponse automatique",
    "respuesta automática",
    "risposta automatica",
    "resposta automática",
];

/// One message from the Sent folder, as the app hands it over.
#[derive(Clone, PartialEq, Eq)]
pub struct SentMessage {
    /// The provider key. Names an exemplar's source; never sent anywhere.
    pub key: String,
    /// When it was sent, in seconds since the Unix epoch.
    pub sent_at: Option<i64>,
    /// The first recipient's address, lower case.
    pub recipient: Option<String>,
    /// The subject.
    pub subject: String,
    /// The body as plain text.
    pub body: String,
    /// Whether it carries a calendar answer.
    pub calendar: bool,
}

impl fmt::Debug for SentMessage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SentMessage")
            .field("sent_at", &self.sent_at)
            .field("body_len", &self.body.len())
            .field("calendar", &self.calendar)
            .finish_non_exhaustive()
    }
}

/// What the corpus needs besides the messages.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct CorpusOptions {
    /// The plain text of every signature the account has used, to remove where a client wrote
    /// no delimiter above it.
    pub signatures: Vec<String>,
    /// The oldest sent message this device holds, whatever the range asked for.
    pub horizon: Option<i64>,
}

impl fmt::Debug for CorpusOptions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CorpusOptions")
            .field("signatures", &self.signatures.len())
            .field("horizon", &self.horizon)
            .finish()
    }
}

/// One message as it enters a prompt: its author's own words.
#[derive(Clone, PartialEq, Eq)]
pub struct CorpusMessage {
    /// The provider key it came from.
    pub key: String,
    /// The author's own words, cut to the per-message cap.
    pub text: String,
    /// The first recipient, lower case.
    pub recipient: Option<String>,
    /// When it was sent.
    pub sent_at: Option<i64>,
}

impl fmt::Debug for CorpusMessage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CorpusMessage")
            .field("text_len", &self.text.len())
            .field("sent_at", &self.sent_at)
            .finish_non_exhaustive()
    }
}

/// The sample per language, and what was found on the way to it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Corpus {
    /// The sampled messages per ISO 639-1 code, oldest first.
    pub languages: BTreeMap<String, Vec<CorpusMessage>>,
    /// What was found.
    pub report: CorpusReport,
}

/// What the corpus builder found, for the screen that asks before anything is sent.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CorpusReport {
    /// Messages handed in.
    pub found: u32,
    /// Messages with enough of their author's own words to learn from, in any language.
    pub usable: u32,
    /// Usable messages in a language the detector does not name.
    pub undetected: u32,
    /// Per language, most messages first.
    pub languages: Vec<LanguageReport>,
    /// The oldest usable message.
    pub oldest: Option<i64>,
    /// The newest usable message.
    pub newest: Option<i64>,
    /// The oldest sent message on this device, whatever the range.
    pub horizon: Option<i64>,
}

/// One language's share of the corpus.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LanguageReport {
    /// ISO 639-1 code.
    pub language: String,
    /// Usable messages in it.
    pub usable: u32,
    /// How many the sample holds.
    pub sampled: u32,
    /// Roughly how many tokens the sample is.
    pub tokens: u32,
}

/// Builds the corpus from `messages`.
#[must_use]
pub fn build(messages: Vec<SentMessage>, options: &CorpusOptions) -> Corpus {
    let stripper = strip::Stripper::new();
    let mut report = CorpusReport {
        found: count(messages.len()),
        horizon: options.horizon,
        ..CorpusReport::default()
    };
    let mut seen = HashSet::new();
    let mut by_language: BTreeMap<String, Vec<CorpusMessage>> = BTreeMap::new();
    for message in messages {
        if message.calendar || is_automatic(&message.subject) {
            continue;
        }
        let text = stripper.own_text(&message.body, &options.signatures);
        if text.split_whitespace().count() < MIN_WORDS || !seen.insert(duplicate_key(&text)) {
            continue;
        }
        report.usable += 1;
        if let Some(sent_at) = message.sent_at {
            report.oldest = Some(report.oldest.map_or(sent_at, |oldest| oldest.min(sent_at)));
            report.newest = Some(report.newest.map_or(sent_at, |newest| newest.max(sent_at)));
        }
        let Some(language) = language::detect(&text) else {
            report.undetected += 1;
            continue;
        };
        by_language
            .entry(language.to_owned())
            .or_default()
            .push(CorpusMessage {
                key: message.key,
                text,
                recipient: message.recipient,
                sent_at: message.sent_at,
            });
    }

    let mut languages = BTreeMap::new();
    for (language, candidates) in by_language {
        let usable = count(candidates.len());
        let sampled = sample::within_budget(candidates, LANGUAGE_BUDGET_TOKENS);
        report.languages.push(LanguageReport {
            language: language.clone(),
            usable,
            sampled: count(sampled.len()),
            tokens: count(
                sampled
                    .iter()
                    .map(|message| estimate_tokens(&message.text))
                    .sum(),
            ),
        });
        languages.insert(language, sampled);
    }
    report
        .languages
        .sort_by(|a, b| b.usable.cmp(&a.usable).then(a.language.cmp(&b.language)));
    Corpus { languages, report }
}

fn is_automatic(subject: &str) -> bool {
    let subject = subject.trim().to_lowercase();
    AUTOMATIC_SUBJECTS
        .iter()
        .any(|opening| subject.starts_with(opening))
}

/// The first characters of `text` with case and spacing flattened, so the same message sent to
/// several people counts once.
fn duplicate_key(text: &str) -> String {
    text.split_whitespace()
        .flat_map(|word| word.chars().chain([' ']))
        .flat_map(char::to_lowercase)
        .take(DUPLICATE_PREFIX_CHARS)
        .collect()
}

fn count(value: usize) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}

#[cfg(test)]
#[path = "corpus_tests.rs"]
mod tests;
