//! What the person did with drafts they were given: one entry per reply sent from an AI draft,
//! kept on the device and never synced (`docs/ai.md`).
//!
//! Two uses, kept apart because only one of them may be capped. The `Message-ID` goes into
//! [`WritingStyleObservations::drafted`], which keeps the sent message out of every later learning
//! run, so a style is never learned from a model's words; it is never capped, because an entry
//! that fell out would make that message learnable again. The difference between the draft and
//! what was sent is what a later refinement of the style learns from; those entries hold the
//! person's words, so they are capped and the oldest go first.
//!
//! **Privacy.** The added and removed runs are the person's words and the model's; they are
//! never logged, which is why [`AiAssistedSend`]'s `Debug` prints counts.

use std::{
    collections::BTreeMap,
    fmt, fs, io,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::preferences::write_atomically;

/// The most [`AiAssistedSend`] entries kept.
const CAP: usize = 200;

/// One reply sent from an AI draft.
#[derive(Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AiAssistedSend {
    /// The sent message's `Message-ID`, without angle brackets.
    pub message_id: String,
    /// The account it was sent from.
    pub account: String,
    /// The style the draft was written in.
    pub style: String,
    /// The language the draft was written in.
    #[serde(default)]
    pub language: String,
    /// When it was sent, in seconds since the Unix epoch.
    pub sent_at: i64,
    /// The share of words the person changed: zero for a draft sent as it was.
    pub changed: f32,
    /// Runs of words the person added.
    #[serde(default)]
    pub added: Vec<String>,
    /// Runs of words the person took out.
    #[serde(default)]
    pub removed: Vec<String>,
}

impl fmt::Debug for AiAssistedSend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AiAssistedSend")
            .field("sent_at", &self.sent_at)
            .field("changed", &self.changed)
            .field("added", &self.added.len())
            .field("removed", &self.removed.len())
            .finish_non_exhaustive()
    }
}

/// The log.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct WritingStyleObservations {
    /// Every reply sent from an AI draft, by `Message-ID`, with the account it was sent from.
    #[serde(default)]
    pub drafted: BTreeMap<String, String>,
    /// What the person changed in each draft, oldest first. Capped.
    #[serde(default)]
    pub sends: Vec<AiAssistedSend>,
}

impl WritingStyleObservations {
    /// Notes `send` as drafted and appends it, dropping the oldest entries beyond the cap.
    pub fn record(&mut self, send: AiAssistedSend) {
        self.drafted
            .insert(send.message_id.clone(), send.account.clone());
        self.sends.push(send);
        if self.sends.len() > CAP {
            let excess = self.sends.len() - CAP;
            self.sends.drain(..excess);
        }
    }

    /// Whether the message with this `Message-ID` was sent from an AI draft.
    #[must_use]
    pub fn is_assisted(&self, message_id: &str) -> bool {
        self.drafted.contains_key(message_id)
    }

    /// Drops what the person changed in drafts written in `style`; called when the style is
    /// forgotten. The replies stay excluded from learning: they were drafted all the same.
    pub fn forget_style(&mut self, style: &str) {
        self.sends.retain(|send| send.style != style);
    }

    /// Drops every entry for `account`; called when the account is removed.
    pub fn forget_account(&mut self, account: &str) {
        self.drafted.retain(|_, sent_from| sent_from != account);
        self.sends.retain(|send| send.account != account);
    }
}

/// The log's file name, beside `preferences.toml`.
const FILE_NAME: &str = "writing_style_observations.toml";

/// The log's path inside the app data directory `base`.
#[must_use]
pub fn writing_style_observations_path(base: impl AsRef<Path>) -> PathBuf {
    base.as_ref().join(FILE_NAME)
}

/// Loads the log; empty when the file is absent or unreadable.
///
/// A log written before `drafted` existed carries its replies only in `sends`, so each is noted
/// as drafted here.
#[must_use]
pub fn load_writing_style_observations(path: impl AsRef<Path>) -> WritingStyleObservations {
    let mut log: WritingStyleObservations = fs::read_to_string(path)
        .ok()
        .and_then(|body| toml::from_str(&body).ok())
        .unwrap_or_default();
    for send in &log.sends {
        log.drafted
            .entry(send.message_id.clone())
            .or_insert_with(|| send.account.clone());
    }
    log
}

/// Writes the log, creating its directory when needed.
///
/// # Errors
///
/// Returns an [`io::Error`] when the directory or the file cannot be written, or
/// [`io::ErrorKind::InvalidData`] when the log does not serialise.
pub fn save_writing_style_observations(
    path: impl AsRef<Path>,
    observations: &WritingStyleObservations,
) -> io::Result<()> {
    let body = toml::to_string(observations)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
    write_atomically(path.as_ref(), &body)
}

#[cfg(test)]
mod tests {
    use super::{
        AiAssistedSend, CAP, WritingStyleObservations, load_writing_style_observations,
        save_writing_style_observations, writing_style_observations_path,
    };

    fn send(message_id: &str, style: &str) -> AiAssistedSend {
        AiAssistedSend {
            message_id: message_id.to_owned(),
            account: "acct".to_owned(),
            style: style.to_owned(),
            language: "en".to_owned(),
            sent_at: 1,
            changed: 0.25,
            added: vec!["secret words".to_owned()],
            removed: Vec::new(),
        }
    }

    #[test]
    fn the_log_round_trips_capping_the_changes_but_never_the_drafted_replies() {
        let dir = std::env::temp_dir().join("mailcal-observations-roundtrip-test");
        let _ = std::fs::remove_dir_all(&dir);
        let path = writing_style_observations_path(&dir);
        let mut log = WritingStyleObservations::default();
        for index in 0..CAP + 5 {
            log.record(send(&format!("m{index}@x"), "work"));
        }
        assert_eq!(log.sends.len(), CAP);
        assert!(
            log.is_assisted("m0@x"),
            "a reply whose changes fell out of the log is still one a draft wrote"
        );
        assert!(log.is_assisted(&format!("m{}@x", CAP + 4)));

        save_writing_style_observations(&path, &log).unwrap();
        assert_eq!(load_writing_style_observations(&path), log);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn forgetting_a_style_keeps_its_replies_excluded_and_an_account_takes_everything() {
        let mut log = WritingStyleObservations::default();
        log.record(send("a@x", "work"));
        log.record(send("b@x", "home"));
        log.forget_style("work");
        assert!(log.sends.iter().all(|send| send.style != "work"));
        assert!(
            log.is_assisted("a@x"),
            "a reply drafted in a forgotten style stays out of learning"
        );
        assert!(log.is_assisted("b@x"));
        log.forget_account("acct");
        assert!(log.sends.is_empty());
        assert!(!log.is_assisted("a@x"));
        assert!(!log.is_assisted("b@x"));
    }

    #[test]
    fn a_log_from_before_the_drafted_list_keeps_its_replies_out_of_learning() {
        let dir = std::env::temp_dir().join("mailcal-observations-legacy-test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = writing_style_observations_path(&dir);
        let legacy = WritingStyleObservations {
            drafted: std::collections::BTreeMap::new(),
            sends: vec![send("old@x", "work")],
        };
        std::fs::write(&path, toml::to_string(&legacy).unwrap()).unwrap();
        assert!(load_writing_style_observations(&path).is_assisted("old@x"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_entry_prints_no_words() {
        assert!(!format!("{:?}", send("a@x", "work")).contains("secret"));
    }
}
