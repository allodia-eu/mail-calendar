//! The importance a composition's message goes out with, held by the core
//! (`docs/importance.md`).
//!
//! A child of [`super`], whose `impl App` block this one continues.
//!
//! The composer draws [`ComposerImportance`] and changes it only through
//! [`App::set_composer_importance`]. Every save and the send read it from the composition, so the
//! importance on screen and the one that goes out cannot differ, and a stored draft reopens with
//! the one it was saved with.

use engine_api::{Importance, Provider};

use crate::{App, CompositionId};

/// What the picker offers, in the order it lists them: highest first, as every mail client that
/// offers one lists it.
const OPTIONS: [Importance; 3] = [Importance::High, Importance::Normal, Importance::Low];

/// A composer's importance picker, as the core holds it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComposerImportance {
    /// What the message goes out with.
    pub selected: Importance,
    /// What the picker offers, in the order it lists them.
    pub options: Vec<Importance>,
    /// Whether it differs from what the composer opened with: the importance's half of "was
    /// anything written".
    pub changed: bool,
}

impl ComposerImportance {
    fn of(selected: Importance, opened_with: Importance) -> Self {
        Self {
            selected,
            options: OPTIONS.to_vec(),
            changed: selected != opened_with,
        }
    }
}

impl<P: Provider> App<P> {
    /// The importance picker `composition`'s composer draws. A composition the core was never told
    /// of goes out normal.
    #[must_use]
    pub fn composer_importance(&self, composition: &CompositionId) -> ComposerImportance {
        self.joined(Some(composition), |open| {
            ComposerImportance::of(open.importance, open.opened_importance)
        })
        .unwrap_or_else(|| ComposerImportance::of(Importance::Normal, Importance::Normal))
    }

    /// Sets the importance `composition`'s message goes out with, and answers with the picker as
    /// it now stands. A composition that has gone (picked as it was sent or discarded) changes
    /// nothing.
    pub fn set_composer_importance(
        &self,
        composition: &CompositionId,
        importance: Importance,
    ) -> ComposerImportance {
        let mut state = self.drafts.lock().expect("drafts mutex poisoned");
        let Some(open) = state.open.get_mut(composition) else {
            return ComposerImportance::of(Importance::Normal, Importance::Normal);
        };
        open.importance = importance;
        ComposerImportance::of(open.importance, open.opened_importance)
    }
}
