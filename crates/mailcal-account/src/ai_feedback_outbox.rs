//! Feedback on drafted replies, kept on the device until Allodia's service has taken it
//! (`docs/ai.md`, "Feedback").
//!
//! Each item is the JSON document `mailcal-ai` built, stored as a string so this file never models
//! that schema. It holds what the draft said and the message it answered only when the person
//! ticked the box that sends them. The outbox is capped and the oldest items go first; signing out
//! of the Allodia account empties it.
//!
//! **Privacy.** A body is feedback and may be mail; [`AiFeedbackItem`]'s `Debug` prints its length.

use std::{
    fmt, fs, io,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

/// The most items kept.
pub const AI_FEEDBACK_CAP: usize = 50;

/// One piece of feedback waiting to be sent.
#[derive(Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiFeedbackItem {
    /// The item's id, also the document's, so the service can recognise one sent twice.
    pub id: String,
    /// When it was given, in seconds since the Unix epoch.
    pub created_at: i64,
    /// Whether the body carries the message and the draft.
    #[serde(default)]
    pub with_content: bool,
    /// The request body, as JSON.
    pub body: String,
}

impl fmt::Debug for AiFeedbackItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AiFeedbackItem")
            .field("created_at", &self.created_at)
            .field("with_content", &self.with_content)
            .field("body_len", &self.body.len())
            .finish_non_exhaustive()
    }
}

/// The outbox.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiFeedbackOutbox {
    /// Oldest first.
    #[serde(default)]
    pub items: Vec<AiFeedbackItem>,
}

impl AiFeedbackOutbox {
    /// Appends `item`, dropping the oldest beyond [`AI_FEEDBACK_CAP`].
    pub fn push(&mut self, item: AiFeedbackItem) {
        self.items.push(item);
        if self.items.len() > AI_FEEDBACK_CAP {
            let excess = self.items.len() - AI_FEEDBACK_CAP;
            self.items.drain(..excess);
        }
    }

    /// Removes the item `id`. Returns whether there was one.
    pub fn remove(&mut self, id: &str) -> bool {
        let before = self.items.len();
        self.items.retain(|item| item.id != id);
        self.items.len() != before
    }
}

/// The outbox's file name, beside `preferences.toml`.
const FILE_NAME: &str = "ai_feedback_outbox.toml";

/// The outbox's path inside the app data directory `base`.
#[must_use]
pub fn ai_feedback_outbox_path(base: impl AsRef<Path>) -> PathBuf {
    base.as_ref().join(FILE_NAME)
}

/// Loads the outbox; empty when the file is absent or unreadable.
#[must_use]
pub fn load_ai_feedback_outbox(path: impl AsRef<Path>) -> AiFeedbackOutbox {
    fs::read_to_string(path)
        .ok()
        .and_then(|body| toml::from_str(&body).ok())
        .unwrap_or_default()
}

/// Writes the outbox, creating its directory when needed; an empty one removes the file, so
/// nothing of it is left behind once the last item has gone.
///
/// # Errors
///
/// Returns an [`io::Error`] when the directory or the file cannot be written or removed, or
/// [`io::ErrorKind::InvalidData`] when the outbox does not serialise.
pub fn save_ai_feedback_outbox(
    path: impl AsRef<Path>,
    outbox: &AiFeedbackOutbox,
) -> io::Result<()> {
    let path = path.as_ref();
    if outbox.items.is_empty() {
        return match fs::remove_file(path) {
            Err(err) if err.kind() != io::ErrorKind::NotFound => Err(err),
            _ => Ok(()),
        };
    }
    let body =
        toml::to_string(outbox).map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, body)
}

#[cfg(test)]
mod tests {
    use super::{
        AI_FEEDBACK_CAP, AiFeedbackItem, AiFeedbackOutbox, ai_feedback_outbox_path,
        load_ai_feedback_outbox, save_ai_feedback_outbox,
    };

    fn item(id: &str) -> AiFeedbackItem {
        AiFeedbackItem {
            id: id.to_owned(),
            created_at: 1,
            with_content: true,
            body: "{\"message\": \"secret words\"}".to_owned(),
        }
    }

    #[test]
    fn the_outbox_round_trips_keeps_the_newest_and_leaves_no_file_once_empty() {
        let dir = std::env::temp_dir().join("mailcal-ai-feedback-outbox-test");
        let _ = std::fs::remove_dir_all(&dir);
        let path = ai_feedback_outbox_path(&dir);
        let mut outbox = AiFeedbackOutbox::default();
        for index in 0..AI_FEEDBACK_CAP + 3 {
            outbox.push(item(&format!("f{index}")));
        }
        assert_eq!(outbox.items.len(), AI_FEEDBACK_CAP);
        assert_eq!(outbox.items[0].id, "f3");

        save_ai_feedback_outbox(&path, &outbox).unwrap();
        assert_eq!(load_ai_feedback_outbox(&path), outbox);

        assert!(outbox.remove("f3"));
        assert!(!outbox.remove("f3"));
        outbox.items.clear();
        save_ai_feedback_outbox(&path, &outbox).unwrap();
        assert!(!path.exists());
        assert_eq!(load_ai_feedback_outbox(&path), AiFeedbackOutbox::default());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_item_prints_no_body() {
        assert!(!format!("{:?}", item("f1")).contains("secret"));
    }
}
