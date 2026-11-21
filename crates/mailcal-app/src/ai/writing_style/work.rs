//! Changes to the style library from Settings → Writing style, the reveal and the detail
//! (`docs/ai.md` rules 8, 15 and 16; `docs/waiting.md`).
//!
//! A change is queued on the host's thread and run off it, until the library and the slots are
//! written. One runs at a time; one asked for while another runs waits and runs next, and the same
//! change asked for again is dropped, so a double press saves once. What the last change came to
//! is kept until the next one starts.

use std::{collections::VecDeque, fmt};

use engine_api::Provider;
use mailcal_account::WritingStyleId;
use mailcal_ai::StyleGuide;

use super::super::StylesWritten;
use crate::{App, Surface};

/// Why a change to the library was not made. The client words it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WritingStyleRefusal {
    /// The name is empty or only spaces.
    BlankName,
    /// The style was forgotten meanwhile.
    Gone,
}

/// The name a style is stored under: `name` without the spaces around it. The rule a client asks
/// before Save is pressed and the change itself acts on.
///
/// # Errors
///
/// [`WritingStyleRefusal::BlankName`] when nothing is left.
pub fn writing_style_name(name: &str) -> Result<String, WritingStyleRefusal> {
    let name = name.trim();
    if name.is_empty() {
        Err(WritingStyleRefusal::BlankName)
    } else {
        Ok(name.to_owned())
    }
}

/// A change to the library or to an account's slot.
#[derive(Clone, PartialEq, Eq)]
pub enum WritingStyleChange {
    /// Renames a style.
    Rename {
        /// The style.
        id: WritingStyleId,
        /// The name as typed.
        name: String,
    },
    /// Replaces a style's notes.
    Notes {
        /// The style.
        id: WritingStyleId,
        /// The notes as typed.
        notes: String,
    },
    /// Sets the style an account drafts in, or none.
    Assign {
        /// The account's id.
        account: String,
        /// The style, or `None`.
        style: Option<WritingStyleId>,
    },
    /// Forgets a style and clears every slot holding it.
    Forget {
        /// The style.
        id: WritingStyleId,
    },
    /// Leaves the reveal by Save: the name and the notes as typed, and the reveal shown.
    SaveReveal {
        /// The style.
        id: WritingStyleId,
        /// The name as typed.
        name: String,
        /// The notes as typed.
        notes: String,
    },
    /// Leaves the reveal without saving: the reveal counts as shown.
    CloseReveal {
        /// The style.
        id: WritingStyleId,
    },
}

/// Which change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WritingStyleChangeKind {
    /// A rename.
    Rename,
    /// New notes.
    Notes,
    /// An account's slot.
    Assign,
    /// Forgetting a style.
    Forget,
    /// The reveal left by Save.
    SaveReveal,
    /// The reveal left without saving.
    CloseReveal,
}

/// A change running or waiting: which change, and the style or account it names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WritingStyleWork {
    /// Which change.
    pub kind: WritingStyleChangeKind,
    /// The style it changes; `None` for an account's slot.
    pub style: Option<WritingStyleId>,
    /// The account whose slot it sets; `None` for a change to a style.
    pub account: Option<String>,
}

/// What a change came to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WritingStyleResult {
    /// Made, and written.
    Done,
    /// Not made, for this reason.
    Refused {
        /// Why.
        reason: WritingStyleRefusal,
    },
    /// Made, and a file could not be written, so it holds until the app closes. The failed-save
    /// notice says so and offers Try again.
    NotWritten,
}

/// The last change and what it came to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WritingStyleOutcome {
    /// Grows with each change, so a client acts on each outcome once.
    pub serial: u64,
    /// The change.
    pub work: WritingStyleWork,
    /// What it came to.
    pub result: WritingStyleResult,
}

/// Names and notes are the person's own, so neither is printed (`docs/logging.md`).
impl fmt::Debug for WritingStyleChange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WritingStyleChange")
            .field("work", &self.work())
            .finish_non_exhaustive()
    }
}

impl WritingStyleChange {
    fn work(&self) -> WritingStyleWork {
        let style = |kind, id: &WritingStyleId| WritingStyleWork {
            kind,
            style: Some(id.clone()),
            account: None,
        };
        match self {
            Self::Rename { id, .. } => style(WritingStyleChangeKind::Rename, id),
            Self::Notes { id, .. } => style(WritingStyleChangeKind::Notes, id),
            Self::Forget { id } => style(WritingStyleChangeKind::Forget, id),
            Self::SaveReveal { id, .. } => style(WritingStyleChangeKind::SaveReveal, id),
            Self::CloseReveal { id } => style(WritingStyleChangeKind::CloseReveal, id),
            Self::Assign { account, .. } => WritingStyleWork {
                kind: WritingStyleChangeKind::Assign,
                style: None,
                account: Some(account.clone()),
            },
        }
    }
}

/// The changes running and waiting, and what the last one came to.
#[derive(Default)]
pub(crate) struct Work {
    running: Option<WritingStyleChange>,
    waiting: VecDeque<WritingStyleChange>,
    pub(super) outcome: Option<WritingStyleOutcome>,
    serial: u64,
}

impl Work {
    /// The change running, then those waiting.
    pub(super) fn listed(&self) -> Vec<WritingStyleWork> {
        self.running
            .iter()
            .chain(&self.waiting)
            .map(WritingStyleChange::work)
            .collect()
    }

    /// Queues `change`. `Some(true)` when the caller is to run it, `Some(false)` when it waits,
    /// `None` when the same change is already running or waiting.
    fn queue(&mut self, change: WritingStyleChange) -> Option<bool> {
        if self.running.as_ref() == Some(&change) || self.waiting.contains(&change) {
            return None;
        }
        if self.running.is_some() {
            self.waiting.push_back(change);
            return Some(false);
        }
        self.outcome = None;
        self.running = Some(change);
        Some(true)
    }

    /// Records what the running change came to and starts the next.
    fn finish(&mut self, result: WritingStyleResult) -> Option<WritingStyleChange> {
        if let Some(done) = self.running.take() {
            self.serial += 1;
            self.outcome = Some(WritingStyleOutcome {
                serial: self.serial,
                work: done.work(),
                result,
            });
        }
        self.running = self.waiting.pop_front();
        if self.running.is_some() {
            self.outcome = None;
        }
        self.running.clone()
    }
}

impl<P: Provider> App<P> {
    fn style_work(&self) -> std::sync::MutexGuard<'_, Work> {
        self.ai
            .writing_style
            .work
            .lock()
            .expect("writing style work mutex poisoned")
    }

    /// The changes running and waiting, and what the last came to.
    pub(super) fn style_work_view(&self) -> (Vec<WritingStyleWork>, Option<WritingStyleOutcome>) {
        let work = self.style_work();
        (work.listed(), work.outcome.clone())
    }

    /// Queues a change to the library and signals [`Surface::WritingStyle`]. Whether the caller
    /// is to run [`App::run_writing_style_changes`] off its own thread: nothing else was running.
    /// Returns at once. A change while the feature is not offered is dropped: no page is drawn to
    /// say what it came to.
    pub fn queue_writing_style_change(&self, change: WritingStyleChange) -> bool {
        if !self.ai_offer() {
            return false;
        }
        let Some(run) = self.style_work().queue(change) else {
            return false;
        };
        self.observer.surface_changed(Surface::WritingStyle);
        run
    }

    /// Runs the change [`App::queue_writing_style_change`] started, and every one queued behind
    /// it, each until the library and the slots are written. Not for a UI thread.
    pub fn run_writing_style_changes(&self) {
        let mut next = self.style_work().running.clone();
        while let Some(change) = next {
            let result = self.apply_writing_style_change(change);
            next = self.style_work().finish(result);
            self.observer.surface_changed(Surface::WritingStyle);
        }
    }

    /// Queues `change` and runs it on this thread: for tests. Not for a UI thread.
    pub fn change_writing_style_now(&self, change: WritingStyleChange) {
        if self.queue_writing_style_change(change) {
            self.run_writing_style_changes();
        }
    }

    fn apply_writing_style_change(&self, change: WritingStyleChange) -> WritingStyleResult {
        let refused = |reason| WritingStyleResult::Refused { reason };
        let edited = match change {
            WritingStyleChange::Rename { id, name } => match writing_style_name(&name) {
                Ok(name) => self.edit_style(&id, |style, _| style.name = name),
                Err(reason) => return refused(reason),
            },
            WritingStyleChange::Notes { id, notes } => {
                self.edit_style(&id, |_, guide| notes.trim().clone_into(&mut guide.notes))
            }
            WritingStyleChange::SaveReveal { id, name, notes } => match writing_style_name(&name) {
                Ok(name) => {
                    let edited = self.edit_style(&id, |style, guide| {
                        style.name = name;
                        style.revealed = true;
                        notes.trim().clone_into(&mut guide.notes);
                    });
                    self.reveal_left(&id);
                    edited
                }
                Err(reason) => return refused(reason),
            },
            WritingStyleChange::CloseReveal { id } => {
                let edited = self.edit_style(&id, |style, _| style.revealed = true);
                self.reveal_left(&id);
                edited
            }
            WritingStyleChange::Assign { account, style } => {
                if style
                    .as_ref()
                    .is_some_and(|id| self.writing_style(id).is_none())
                {
                    return refused(WritingStyleRefusal::Gone);
                }
                self.assign_writing_style(&account, style);
                true
            }
            WritingStyleChange::Forget { id } => {
                self.forget_writing_style(&id);
                true
            }
        };
        if !edited {
            return refused(WritingStyleRefusal::Gone);
        }
        match self.settle_writing_styles() {
            StylesWritten::Written => WritingStyleResult::Done,
            StylesWritten::NotWritten => WritingStyleResult::NotWritten,
        }
    }

    /// Changes the stored style `id` and the description it carries. Whether it was there. A
    /// description that cannot be read is written back as it was, never as an empty one.
    fn edit_style(
        &self,
        id: &WritingStyleId,
        edit: impl FnOnce(&mut mailcal_account::StoredWritingStyle, &mut StyleGuide),
    ) -> bool {
        let Some(mut style) = self.writing_style(id) else {
            return false;
        };
        let read = serde_json::from_str::<StyleGuide>(&style.guide_json).ok();
        let mut guide = read.clone().unwrap_or_default();
        edit(&mut style, &mut guide);
        if read.is_some()
            && let Ok(json) = serde_json::to_string(&guide)
        {
            style.guide_json = json;
        }
        self.replace_writing_style(id, style)
    }
}
