//! The replies this device sent from a draft the assistant wrote (`docs/ai.md` rule 25), kept so
//! a style is never learned from a model's words.
//!
//! The `Message-ID` of each, with the account it went from. Never capped, because an entry that
//! fell out would make that message learnable again; kept when a style is forgotten, because the
//! reply was drafted all the same; gone with its account.

use std::{
    collections::BTreeMap,
    fmt, fs, io,
    path::{Path, PathBuf},
};

use engine_api::MessageIdHeader;
use serde::{Deserialize, Serialize};

/// The log: each drafted reply's `Message-ID` and the id of the account it was sent from.
#[derive(Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DraftedReplies {
    /// Every drafted reply, by `Message-ID` without its angle brackets, with its account's id.
    #[serde(default)]
    pub drafted: BTreeMap<MessageIdHeader, String>,
}

impl DraftedReplies {
    /// Notes the reply `message_id`, sent from `account`, as drafted.
    pub fn note(&mut self, message_id: MessageIdHeader, account: &str) {
        self.drafted.insert(message_id, account.to_owned());
    }

    /// Whether the message `message_id` was sent from a draft.
    #[must_use]
    pub fn is_drafted(&self, message_id: &MessageIdHeader) -> bool {
        self.drafted.contains_key(message_id)
    }

    /// Drops every reply sent from `account`; called when the account is removed. Whether any was.
    pub fn forget_account(&mut self, account: &str) -> bool {
        let before = self.drafted.len();
        self.drafted.retain(|_, sent_from| sent_from != account);
        self.drafted.len() != before
    }
}

/// A `Message-ID` names the sender's domain, so the log prints as its length.
impl fmt::Debug for DraftedReplies {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DraftedReplies")
            .field("drafted", &self.drafted.len())
            .finish()
    }
}

/// The log's file name, beside `preferences.toml`.
const FILE_NAME: &str = "drafted_replies.toml";

/// The log's path inside the app data directory `base`.
#[must_use]
pub fn drafted_replies_path(base: impl AsRef<Path>) -> PathBuf {
    base.as_ref().join(FILE_NAME)
}

/// The log at `path`; empty when the file is missing or does not parse.
#[must_use]
pub fn load_drafted_replies(path: impl AsRef<Path>) -> DraftedReplies {
    fs::read_to_string(path)
        .ok()
        .and_then(|body| toml::from_str(&body).ok())
        .unwrap_or_default()
}

/// Writes the log to `path`, beside it and renamed over it.
///
/// # Errors
///
/// Returns an [`io::Error`] when the directory or the file cannot be written, or
/// [`io::ErrorKind::InvalidData`] when the log does not serialise.
pub fn save_drafted_replies(path: impl AsRef<Path>, log: &DraftedReplies) -> io::Result<()> {
    let body =
        toml::to_string(log).map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
    crate::preferences::replace_file(path.as_ref(), &body)
}

#[cfg(test)]
mod tests {
    use engine_api::MessageIdHeader;

    use super::{DraftedReplies, drafted_replies_path, load_drafted_replies, save_drafted_replies};

    fn id(value: &str) -> MessageIdHeader {
        MessageIdHeader::new(value).unwrap()
    }

    #[test]
    fn the_log_round_trips_and_an_account_takes_its_replies_with_it() {
        let dir = tempfile::tempdir().unwrap();
        let path = drafted_replies_path(dir.path());
        let mut log = DraftedReplies::default();
        log.note(id("a@example.eu"), "one");
        log.note(id("b@example.eu"), "two");
        save_drafted_replies(&path, &log).unwrap();
        let mut loaded = load_drafted_replies(&path);
        assert_eq!(loaded, log);
        assert!(loaded.forget_account("one"));
        assert!(!loaded.is_drafted(&id("a@example.eu")));
        assert!(loaded.is_drafted(&id("b@example.eu")));
        assert!(!loaded.forget_account("one"));
    }

    #[test]
    fn a_missing_or_broken_file_is_an_empty_log() {
        let dir = tempfile::tempdir().unwrap();
        let path = drafted_replies_path(dir.path());
        assert_eq!(load_drafted_replies(&path), DraftedReplies::default());
        std::fs::write(&path, "[[not toml").unwrap();
        assert_eq!(load_drafted_replies(&path), DraftedReplies::default());
    }

    #[test]
    fn the_log_prints_no_address() {
        let mut log = DraftedReplies::default();
        log.note(id("x@example.eu"), "one");
        assert!(!format!("{log:?}").contains("example"));
    }
}
