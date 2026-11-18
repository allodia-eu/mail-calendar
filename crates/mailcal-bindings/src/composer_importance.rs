//! A message's importance across the FFI: the value every row, the reading view and the composer
//! name, and the composer's picker (`docs/importance.md`).
//!
//! The composition holds its importance in the core, as it holds its files: a composer draws
//! [`ComposerImportance`] and changes it only through [`MailcalApp::set_composer_importance`].
//! Every save and the send read it there, so no client passes one.

use engine_api::Importance;
use mailcal_app::{ComposerImportance as AppComposerImportance, CompositionId};

use crate::{MailcalApp, MailcalError};

/// The importance a message's sender gave it (RFC 2156 `Importance`). Normal is what a message
/// that states none is.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, uniffi::Enum)]
pub enum MessageImportance {
    /// Low importance.
    Low,
    /// Normal importance, or none stated.
    #[default]
    Normal,
    /// High importance.
    High,
}

impl From<Importance> for MessageImportance {
    fn from(importance: Importance) -> Self {
        match importance {
            Importance::Low => Self::Low,
            Importance::Normal => Self::Normal,
            Importance::High => Self::High,
        }
    }
}

impl From<MessageImportance> for Importance {
    fn from(importance: MessageImportance) -> Self {
        match importance {
            MessageImportance::Low => Self::Low,
            MessageImportance::Normal => Self::Normal,
            MessageImportance::High => Self::High,
        }
    }
}

/// A composer's importance picker, as the core holds it.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ComposerImportance {
    /// What the message goes out with: the picker's checked entry.
    pub selected: MessageImportance,
    /// What the picker offers, in the order it lists them. The client words each one.
    pub options: Vec<MessageImportance>,
    /// Whether it differs from what the composer opened with: the importance's half of "was
    /// anything written", beside `ComposerFiles.changed`.
    pub changed: bool,
}

impl From<AppComposerImportance> for ComposerImportance {
    fn from(picker: AppComposerImportance) -> Self {
        Self {
            selected: picker.selected.into(),
            options: picker.options.into_iter().map(Into::into).collect(),
            changed: picker.changed,
        }
    }
}

#[uniffi::export]
impl MailcalApp {
    /// The importance picker `composition`'s composer draws. A composition the core does not hold
    /// goes out normal.
    ///
    /// # Errors
    ///
    /// Returns [`MailcalError::Engine`] if `composition` is blank.
    pub fn composer_importance(
        &self,
        composition: String,
    ) -> Result<ComposerImportance, MailcalError> {
        Ok(self
            .app
            .composer_importance(&composition_id(composition)?)
            .into())
    }

    /// Sets the importance `composition`'s message goes out with, and answers with the picker as
    /// it now stands. The next save and the send carry it.
    ///
    /// # Errors
    ///
    /// Returns [`MailcalError::Engine`] if `composition` is blank.
    pub fn set_composer_importance(
        &self,
        composition: String,
        importance: MessageImportance,
    ) -> Result<ComposerImportance, MailcalError> {
        Ok(self
            .app
            .set_composer_importance(&composition_id(composition)?, importance.into())
            .into())
    }
}

fn composition_id(value: String) -> Result<CompositionId, MailcalError> {
    CompositionId::new(value)
        .ok_or_else(|| MailcalError::Engine("composition id is blank".to_owned()))
}
