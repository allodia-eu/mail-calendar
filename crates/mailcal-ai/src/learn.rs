//! Learning a writing style from a [`Corpus`]: one pass per language.
//!
//! A language whose sample fits one request is described in one request. A larger one is split,
//! each part described on its own, and the partial descriptions merged by one more request, so
//! the model never reads more than [`REQUEST_BUDGET_TOKENS`] of mail at once.
//!
//! The model also names the messages that best show the person's everyday voice. Those become
//! the [`Exemplars`] **verbatim, from the device's own copy**: what comes back from the model is a
//! list of message numbers, never text, so an exemplar is always the person's words and never a
//! model's paraphrase of them.

use std::{
    collections::BTreeMap,
    fmt,
    sync::atomic::{AtomicBool, Ordering},
};

use schemars::JsonSchema;
use serde::Deserialize;

use crate::{
    AiError, Exemplars, GatedBackend, LanguageStyle, Provenance, StyleGuide,
    corpus::{Corpus, CorpusMessage, estimate_tokens},
    prompt::{FENCE_PREAMBLE, fence, language_name},
    tool,
    wire::{ChatMessage, ChatRequest, Metering, Purpose},
};

/// The most mail one request reads, in tokens.
pub const REQUEST_BUDGET_TOKENS: usize = 60_000;
/// The most passages kept per language.
const EXEMPLARS_PER_LANGUAGE: usize = 10;
/// The fewest passages the fallback keeps when a model names none.
const FALLBACK_EXEMPLARS: usize = 6;
/// The longest passage kept, in characters.
const EXEMPLAR_CHARS: usize = 900;
/// The answer's length cap for a description.
const ANSWER_TOKENS: u32 = 4_000;

/// How far a learning run has got.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LearnProgress {
    /// Requests answered.
    pub done: u32,
    /// Requests the run will make.
    pub total: u32,
}

/// What a learning run needs besides the corpus.
pub struct LearnOptions<'a> {
    /// The language the person reads the app in; the descriptions are written in it, because
    /// the person reads them back.
    pub ui_language: &'a str,
    /// When the run was started, in seconds since the Unix epoch.
    pub now: i64,
    /// Set by the person to stop. Checked before every request; a request in flight runs to its
    /// timeout.
    pub cancel: &'a AtomicBool,
    /// Told before every request and once at the end.
    pub progress: &'a (dyn Fn(LearnProgress) + Sync),
}

impl fmt::Debug for LearnOptions<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LearnOptions")
            .field("ui_language", &self.ui_language)
            .field("now", &self.now)
            .finish_non_exhaustive()
    }
}

/// A learned style, and what learning it cost.
#[derive(Debug, Clone, PartialEq)]
pub struct Learned {
    /// The synced half.
    pub guide: StyleGuide,
    /// The local half.
    pub exemplars: Exemplars,
    /// What the relay charged in all, and the balance after; `None` from an own endpoint.
    pub metering: Option<Metering>,
}

/// A run that stopped, and what it had cost by then.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
#[error("{error}")]
pub struct LearnError {
    /// Why it stopped.
    pub error: AiError,
    /// What the requests before it cost.
    pub metering: Option<Metering>,
}

/// What the model returns for one sample.
#[derive(Deserialize, JsonSchema)]
struct Described {
    /// How the person writes.
    style: LanguageStyle,
    /// The numbers of the six to ten messages that best show their everyday voice.
    #[serde(default)]
    exemplars: Vec<u32>,
}

/// What the model returns when merging.
#[derive(Deserialize, JsonSchema)]
struct Merged {
    /// How the person writes, all parts considered.
    style: LanguageStyle,
}

/// Learns a style from `corpus`.
///
/// # Errors
///
/// Returns a [`LearnError`] carrying the first request's failure, [`AiError::Cancelled`] when the
/// person stopped it, or [`AiError::Malformed`] for a corpus with nothing to learn from; with
/// what the earlier requests cost.
pub fn learn_style(
    corpus: &Corpus,
    backend: &GatedBackend,
    options: &LearnOptions<'_>,
) -> Result<Learned, LearnError> {
    let plans: Vec<(&String, Vec<&[CorpusMessage]>)> = corpus
        .languages
        .iter()
        .filter(|(_, messages)| !messages.is_empty())
        .map(|(language, messages)| (language, chunks(messages)))
        .collect();
    if plans.is_empty() {
        return Err(LearnError {
            error: AiError::Malformed,
            metering: None,
        });
    }
    let total: usize = plans
        .iter()
        .map(|(_, parts)| parts.len() + usize::from(parts.len() > 1))
        .sum();
    let mut run = Run {
        backend,
        options,
        done: 0,
        total: u32::try_from(total).unwrap_or(u32::MAX),
        metering: None,
    };

    let mut guide = StyleGuide::new();
    let mut exemplars = Exemplars::new();
    let mut counts = BTreeMap::new();
    for (language, parts) in plans {
        let (style, passages) = run.language(language, &parts)?;
        guide.languages.insert(language.clone(), style);
        exemplars.languages.insert(language.clone(), passages);
        counts.insert(
            language.clone(),
            u32::try_from(parts.iter().map(|part| part.len()).sum::<usize>()).unwrap_or(u32::MAX),
        );
    }
    (options.progress)(LearnProgress {
        done: run.done,
        total: run.total,
    });
    guide.learned = Some(Provenance {
        oldest: corpus.report.oldest,
        newest: corpus.report.newest,
        messages_per_language: counts,
        learned_at: options.now,
        unknown: serde_json::Map::new(),
    });
    log::info!(
        "ai: learned a style in {} language(s) over {} request(s)",
        guide.languages.len(),
        run.done
    );
    Ok(Learned {
        guide,
        exemplars,
        metering: run.metering,
    })
}

/// One run's requests, and the running total of what they cost.
struct Run<'a> {
    backend: &'a GatedBackend,
    options: &'a LearnOptions<'a>,
    done: u32,
    total: u32,
    metering: Option<Metering>,
}

impl Run<'_> {
    /// One language: each part described, then merged when there was more than one.
    fn language(
        &mut self,
        language: &str,
        parts: &[&[CorpusMessage]],
    ) -> Result<(LanguageStyle, Vec<String>), LearnError> {
        let mut styles = Vec::new();
        let mut passages = Vec::new();
        for part in parts {
            let described: Described =
                self.ask(&describe_request(language, self.options.ui_language, part))?;
            passages.extend(picked(part, &described.exemplars));
            styles.push(described.style.bounded());
        }
        let style = if styles.len() == 1 {
            styles.remove(0)
        } else {
            let merged: Merged =
                self.ask(&merge_request(language, self.options.ui_language, &styles))?;
            merged.style.bounded()
        };
        if passages.is_empty() {
            passages = fallback(parts, style.typical_words);
        }
        passages.truncate(EXEMPLARS_PER_LANGUAGE);
        Ok((style, passages))
    }

    fn ask<T: for<'de> Deserialize<'de>>(
        &mut self,
        request: &ChatRequest,
    ) -> Result<T, LearnError> {
        if self.options.cancel.load(Ordering::Relaxed) {
            return Err(self.stopped(AiError::Cancelled));
        }
        (self.options.progress)(LearnProgress {
            done: self.done,
            total: self.total,
        });
        let response = self
            .backend
            .chat(request)
            .map_err(|error| self.stopped(error))?;
        self.done += 1;
        if let Some(charged) = response.allodia {
            self.metering = Some(charged.after(self.metering));
        }
        tool::read(&response).map_err(|error| self.stopped(error))
    }

    fn stopped(&self, error: AiError) -> LearnError {
        LearnError {
            error,
            metering: self.metering,
        }
    }
}

/// `messages` split into parts of at most [`REQUEST_BUDGET_TOKENS`].
fn chunks(messages: &[CorpusMessage]) -> Vec<&[CorpusMessage]> {
    let mut parts = Vec::new();
    let mut start = 0;
    let mut used = 0;
    for (index, message) in messages.iter().enumerate() {
        let tokens = estimate_tokens(&message.text);
        if used + tokens > REQUEST_BUDGET_TOKENS && index > start {
            parts.push(&messages[start..index]);
            start = index;
            used = 0;
        }
        used += tokens;
    }
    parts.push(&messages[start..]);
    parts
}

fn describe_request(language: &str, ui_language: &str, part: &[CorpusMessage]) -> ChatRequest {
    let system = format!(
        "You study how one person writes email, so that replies can later be drafted in their \
         voice. You are given {count} emails they wrote in {language}, numbered. {FENCE_PREAMBLE}\n\
         \n\
         Describe their style by calling emit_json once. Be concrete and specific to this \
         person; leave out anything that would describe most writers.\n\
         - greetings and sign_offs: the exact wording they use, most frequent first, each with \
         the rough share of emails that use it.\n\
         - signs_as: the name they sign with, exactly as written, or empty.\n\
         - register: how formal they are and how that shifts with the recipient (formal or \
         informal pronouns, first names or titles).\n\
         - typical_words: the typical length of their emails, in words.\n\
         - shape, punctuation, structure, moves: short descriptions.\n\
         - phrases: short expressions characteristic of them, a few words each.\n\
         - avoid: what they never do, which a draft in their name must not do either.\n\
         - exemplars: the numbers of the six to ten emails that best show their everyday \
         voice, varied in recipient and purpose.\n\
         \n\
         Write register, shape, punctuation, structure, moves and avoid in {ui}. Quote \
         greetings, sign-offs and phrases exactly as the person writes them. Never copy a \
         sentence of an email into a description.",
        count = part.len(),
        language = language_name(language),
        ui = language_name(ui_language),
    );
    let mail = part
        .iter()
        .enumerate()
        .map(|(index, message)| fence(&format!("number=\"{}\"", index + 1), &message.text))
        .collect::<Vec<_>>()
        .join("\n\n");
    let (tool, choice) =
        tool::forced::<Described>("Record the description of how this person writes.");
    ChatRequest {
        purpose: Purpose::Style,
        messages: vec![ChatMessage::system(system), ChatMessage::user(mail)],
        tools: vec![tool],
        tool_choice: Some(choice),
        temperature: Some(0.2),
        max_tokens: Some(ANSWER_TOKENS),
    }
}

fn merge_request(language: &str, ui_language: &str, styles: &[LanguageStyle]) -> ChatRequest {
    let system = format!(
        "You are given {count} descriptions of how one person writes email in {language}, each \
         made from a different sample of their mail. Merge them into one by calling emit_json: \
         keep what recurs, reconcile the shares, and leave out what only one sample shows. Keep \
         the descriptions in {ui} and the quoted wording exactly as given.",
        count = styles.len(),
        language = language_name(language),
        ui = language_name(ui_language),
    );
    let parts = serde_json::to_string_pretty(styles).unwrap_or_default();
    let (tool, choice) = tool::forced::<Merged>("Record the merged description.");
    ChatRequest {
        purpose: Purpose::Style,
        messages: vec![ChatMessage::system(system), ChatMessage::user(parts)],
        tools: vec![tool],
        tool_choice: Some(choice),
        temperature: Some(0.2),
        max_tokens: Some(ANSWER_TOKENS),
    }
}

/// The passages the model named, from the device's own copy. Numbers outside the part are
/// ignored; a model that miscounts gets fewer exemplars, never someone else's text.
fn picked(part: &[CorpusMessage], numbers: &[u32]) -> Vec<String> {
    let mut seen = Vec::new();
    numbers
        .iter()
        .filter_map(|number| usize::try_from(*number).ok()?.checked_sub(1))
        .filter(|index| *index < part.len())
        .filter(|index| {
            let fresh = !seen.contains(index);
            seen.push(*index);
            fresh
        })
        .map(|index| passage(&part[index].text))
        .collect()
}

/// Passages for a guide that arrived from another device, picked on this one without a request.
///
/// A synced guide carries none, so each device picks its own from its own sent mail: for each of
/// the guide's languages, the messages in `corpus` closest to that language's typical length, as
/// learning does when a model names none. A language `corpus` has no mail in gets no passages; a
/// draft in it works from the guide and the recipient context.
#[must_use]
pub fn pick_passages(guide: &StyleGuide, corpus: &Corpus) -> Exemplars {
    let mut exemplars = Exemplars::new();
    for (language, style) in &guide.languages {
        let Some(messages) = corpus
            .languages
            .get(language)
            .filter(|messages| !messages.is_empty())
        else {
            continue;
        };
        exemplars.languages.insert(
            language.clone(),
            fallback(&[messages.as_slice()], style.typical_words),
        );
    }
    exemplars
}

/// When a model named no exemplars: the messages closest to the person's typical length.
fn fallback(parts: &[&[CorpusMessage]], typical_words: u32) -> Vec<String> {
    let typical = usize::try_from(typical_words).unwrap_or(usize::MAX);
    let mut messages: Vec<&CorpusMessage> = parts.iter().flat_map(|part| part.iter()).collect();
    messages.sort_by_key(|message| message.text.split_whitespace().count().abs_diff(typical));
    messages
        .into_iter()
        .take(FALLBACK_EXEMPLARS)
        .map(|message| passage(&message.text))
        .collect()
}

/// A message cut to [`EXEMPLAR_CHARS`] at a paragraph when one allows it.
fn passage(text: &str) -> String {
    if text.chars().count() <= EXEMPLAR_CHARS {
        return text.to_owned();
    }
    let head: String = text.chars().take(EXEMPLAR_CHARS).collect();
    match head.rfind("\n\n") {
        Some(at) if at > 0 => head[..at].trim_end().to_owned(),
        _ => head,
    }
}

#[cfg(test)]
#[path = "learn_tests.rs"]
mod tests;
