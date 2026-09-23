//! Writing styles: the library of learned styles, which account drafts in which, the backend AI
//! requests go through, and the learning and drafting use cases (`docs/ai.md`).
//!
//! The library is its own store (`writing_styles.toml`) and the per-account pointer rides in the
//! preferences, as signatures do. The backend is injected by the binding layer after construction
//! ([`App::set_ai_backend`]), because which one exists changes at runtime: a sign-out, an edit of
//! the own endpoint. It is held as a [`GatedBackend`] and nothing else, so no request from here can
//! reach a backend without passing the jurisdiction gate.
//!
//! **Privacy.** A style's guide and passages are user content. Nothing here logs them; counts,
//! stages and labels only (`docs/logging.md`).

use std::{
    path::PathBuf,
    sync::{Arc, Mutex, atomic::AtomicBool},
};

use engine_api::Provider;
use mailcal_account::{
    Preferences, StoredWritingStyle, WritingStyleId, WritingStyles, load_preferences,
    load_writing_styles, save_preferences, save_writing_styles,
};
use mailcal_ai::{Exemplars, GatedBackend, StyleGuide};
use mailcal_viewmodel::{
    AccountWritingStyleRow, AiRoute, GateRefusal, HabitRow, LanguageStyleRow, LearningProgress,
    WritingStyleDetail, WritingStyleRow, WritingStyleSnapshot,
};

use crate::{App, Surface};

mod draft;
mod endpoint;
mod learn;
mod sent;

pub use draft::{DraftReply, ReplyDraftRequest};
pub use learn::{LearnFailure, LearnRange, LearnReport};

/// Why a writing-style use case produced nothing.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum WritingStyleError {
    /// No backend: this build has no Allodia relay the person may use, and no own endpoint is set
    /// up.
    #[error("AI is not available")]
    Unavailable,
    /// A learning run is already going.
    #[error("a learning run is already in progress")]
    Busy,
    /// The account has no Sent folder this device knows of.
    #[error("the account has no Sent folder")]
    NoSentFolder,
    /// Nothing the account sent in the range says enough to learn from.
    #[error("nothing to learn from")]
    NothingToLearn,
    /// No style to draft in: none named, none assigned, or the one named is gone.
    #[error("no writing style")]
    NoStyle,
    /// The message to answer is not on this device.
    #[error("the message was not found")]
    NotFound,
    /// The backend's answer.
    #[error(transparent)]
    Ai(#[from] mailcal_ai::AiError),
}

/// The loaded library, the pointer store, the backend and a learning run's state.
pub(crate) struct WritingStyleState {
    library: Mutex<WritingStyles>,
    library_path: Option<PathBuf>,
    prefs_path: Option<PathBuf>,
    /// The assignments when there is no preferences file, as [`crate::signatures`] keeps them.
    /// Boxed: a `Preferences` is large, and every future that holds the app by value carries it.
    memory: Mutex<Box<Preferences>>,
    backend: Mutex<Option<Arc<GatedBackend>>>,
    learning: Mutex<Option<LearningProgress>>,
    cancel: AtomicBool,
}

impl WritingStyleState {
    pub(crate) fn new(library_path: Option<PathBuf>, prefs_path: Option<PathBuf>) -> Self {
        Self {
            library: Mutex::new(
                library_path
                    .as_ref()
                    .map(load_writing_styles)
                    .unwrap_or_default(),
            ),
            library_path,
            prefs_path,
            memory: Mutex::new(Box::default()),
            backend: Mutex::new(None),
            learning: Mutex::new(None),
            cancel: AtomicBool::new(false),
        }
    }

    fn library(&self) -> std::sync::MutexGuard<'_, WritingStyles> {
        self.library.lock().expect("writing-style library poisoned")
    }

    fn persist_library(&self, library: &WritingStyles) {
        if let Some(path) = &self.library_path {
            let _ = save_writing_styles(path, library);
        }
    }

    /// Reads the preferences, applies `edit`, and writes them back when it changed anything.
    fn edit_prefs<T>(&self, edit: impl FnOnce(&mut Preferences) -> T) -> T {
        match &self.prefs_path {
            Some(path) => {
                let mut prefs = load_preferences(path);
                let before = prefs.clone();
                let result = edit(&mut prefs);
                if prefs != before {
                    let _ = save_preferences(path, &prefs);
                }
                result
            }
            None => edit(&mut self.memory.lock().expect("writing-style memory poisoned")),
        }
    }

    fn assigned(&self, account: &str) -> Option<WritingStyleId> {
        let id = self.edit_prefs(|prefs| prefs.writing_style_of(account).cloned())?;
        self.library().entries.contains_key(&id).then_some(id)
    }

    fn backend(&self) -> Option<Arc<GatedBackend>> {
        self.backend.lock().expect("ai backend poisoned").clone()
    }
}

/// Mints an opaque style id from the system CSPRNG, as signature ids are.
fn mint_style_id() -> WritingStyleId {
    use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
    use ring::rand::{SecureRandom, SystemRandom};

    let mut bytes = [0u8; 16];
    SystemRandom::new()
        .fill(&mut bytes)
        .expect("system CSPRNG fills 16 bytes");
    WritingStyleId::new(URL_SAFE_NO_PAD.encode(bytes)).expect("base64 is a valid style id")
}

/// A stored style's guide, or an empty one when the stored JSON does not read.
fn guide_of(style: &StoredWritingStyle) -> StyleGuide {
    serde_json::from_str(&style.guide_json).unwrap_or_default()
}

/// A stored style's passages, or none when the stored JSON does not read.
fn exemplars_of(style: &StoredWritingStyle) -> Exemplars {
    serde_json::from_str(&style.exemplars_json).unwrap_or_default()
}

fn row(id: &WritingStyleId, style: &StoredWritingStyle) -> WritingStyleRow {
    let guide = guide_of(style);
    let learned = guide.learned.unwrap_or_default();
    let mut languages: Vec<(String, u32)> = guide
        .languages
        .keys()
        .map(|language| {
            let count = learned
                .messages_per_language
                .get(language)
                .copied()
                .unwrap_or_default();
            (language.clone(), count)
        })
        .collect();
    languages.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    WritingStyleRow {
        id: id.as_str().to_owned(),
        name: style.name.clone(),
        source_account: style.source.clone(),
        messages: languages.iter().map(|(_, count)| count).sum(),
        languages: languages
            .into_iter()
            .map(|(language, _)| language)
            .collect(),
        oldest: learned.oldest,
        newest: learned.newest,
        learned_at: learned.learned_at,
    }
}

fn habits(habits: &[mailcal_ai::Habit]) -> Vec<HabitRow> {
    habits
        .iter()
        .map(|habit| HabitRow {
            text: habit.text.clone(),
            share: habit.share,
        })
        .collect()
}

impl<P: Provider> App<P> {
    /// The Writing style surface (pulled after a [`Surface::WritingStyle`] signal).
    pub async fn writing_styles(&self) -> WritingStyleSnapshot {
        let accounts = self.account_rows().await;
        let state = &self.writing_style;
        let backend = state.backend();
        let styles = state
            .library()
            .ordered()
            .into_iter()
            .map(|(id, style)| row(id, style))
            .collect();
        let accounts = accounts
            .into_iter()
            .map(|account| AccountWritingStyleRow {
                style: state.assigned(&account.id).map(|id| id.as_str().to_owned()),
                account_id: account.id,
                email: account.email,
            })
            .collect();
        WritingStyleSnapshot {
            route: backend.as_ref().map(|backend| match backend.destination() {
                mailcal_ai::Destination::AllodiaRelay => AiRoute::Relay,
                mailcal_ai::Destination::OwnEndpoint { .. } => AiRoute::OwnEndpoint,
            }),
            refused: backend
                .and_then(|backend| backend.check().err())
                .map(|refused| GateRefusal {
                    mode: refused.mode,
                    class: refused.class,
                }),
            styles,
            accounts,
            learning: state
                .learning
                .lock()
                .expect("learning state poisoned")
                .clone(),
        }
    }

    /// One style in full, for the reveal and edit screen; `None` when the id names nothing.
    #[must_use]
    pub fn writing_style_detail(&self, id: &str) -> Option<WritingStyleDetail> {
        let id = WritingStyleId::new(id)?;
        let library = self.writing_style.library();
        let style = library.get(&id)?;
        let guide = guide_of(style);
        let row = row(&id, style);
        let languages = row
            .languages
            .iter()
            .filter_map(|language| {
                let learned = guide.languages.get(language)?;
                Some(LanguageStyleRow {
                    language: language.clone(),
                    greetings: habits(&learned.greetings),
                    sign_offs: habits(&learned.sign_offs),
                    signs_as: learned.signs_as.clone(),
                    register: learned.register.clone(),
                    typical_words: learned.typical_words,
                    shape: learned.shape.clone(),
                    punctuation: learned.punctuation.clone(),
                    structure: learned.structure.clone(),
                    moves: learned.moves.clone(),
                    phrases: learned.phrases.clone(),
                    avoid: learned.avoid.clone(),
                })
            })
            .collect();
        Some(WritingStyleDetail {
            row,
            notes: guide.notes,
            languages,
        })
    }

    /// Renames a style. Returns whether the id named one.
    pub fn rename_writing_style(&self, id: &str, name: String) -> bool {
        self.edit_writing_style(id, |style| style.name = name)
    }

    /// Replaces a style's notes, which every draft in it reads. Returns whether the id named one.
    pub fn update_writing_style_notes(&self, id: &str, notes: String) -> bool {
        self.edit_writing_style(id, |style| {
            let mut guide = guide_of(style);
            guide.notes = notes;
            if let Ok(json) = serde_json::to_string(&guide) {
                style.guide_json = json;
            }
        })
    }

    fn edit_writing_style(&self, id: &str, edit: impl FnOnce(&mut StoredWritingStyle)) -> bool {
        let Some(id) = WritingStyleId::new(id) else {
            return false;
        };
        let state = &self.writing_style;
        let mut library = state.library();
        let Some(style) = library.get_mut(&id) else {
            return false;
        };
        edit(style);
        state.persist_library(&library);
        drop(library);
        self.observer.surface_changed(Surface::WritingStyle);
        true
    }

    /// Forgets a style: removes it from the library and from every account that drafted in it.
    /// Returns whether the id named one.
    pub fn delete_writing_style(&self, id: &str) -> bool {
        let Some(id) = WritingStyleId::new(id) else {
            return false;
        };
        let state = &self.writing_style;
        let mut library = state.library();
        if !library.remove(&id) {
            return false;
        }
        state.persist_library(&library);
        drop(library);
        state.edit_prefs(|prefs| prefs.forget_writing_style(&id));
        log::info!("ai: forgot one writing style");
        self.observer.surface_changed(Surface::WritingStyle);
        true
    }

    /// Assigns a style to `account`, or clears it with `None`. An id naming nothing clears it.
    pub fn set_account_writing_style(&self, account: &str, style: Option<String>) {
        let state = &self.writing_style;
        let style = style
            .and_then(WritingStyleId::new)
            .filter(|id| state.library().entries.contains_key(id));
        state.edit_prefs(|prefs| prefs.set_account_writing_style(account, style));
        self.observer.surface_changed(Surface::WritingStyle);
    }

    /// The style `account` drafts in, when it has one that still exists.
    #[must_use]
    pub fn resolve_writing_style(&self, account: &str) -> Option<String> {
        self.writing_style
            .assigned(account)
            .map(|id| id.as_str().to_owned())
    }

    /// Drops an account's assignment; called from account removal.
    pub(crate) fn remove_account_writing_style(&self, account: &str) {
        self.writing_style
            .edit_prefs(|prefs| prefs.remove_account_writing_style(account));
    }

    /// Stores a newly learned style and assigns it to `account` when the account had none.
    fn store_writing_style(&self, account: &str, style: StoredWritingStyle) -> WritingStyleId {
        let id = mint_style_id();
        let state = &self.writing_style;
        let mut library = state.library();
        library.insert(id.clone(), style);
        state.persist_library(&library);
        drop(library);
        if state.assigned(account).is_none() {
            let assigned = id.clone();
            state.edit_prefs(|prefs| prefs.set_account_writing_style(account, Some(assigned)));
        }
        id
    }
}

#[cfg(test)]
#[path = "writing_style_tests.rs"]
mod tests;
