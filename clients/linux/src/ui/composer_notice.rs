//! What the composer's one error line can say.
//!
//! Its own module so [`super::composer_model`] stays under the line limit, and because the set is
//! a product decision rather than composer state: adding a member means the composer has a new
//! way to fail that the user has to be told about.

/// Which failure the composer's error line is showing.
///
/// A single line carries all of them, so it has to say which it is: "couldn't send this" and
/// "couldn't attach the original's files" ask the user for different things.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ComposerNotice {
    /// The message could not be prepared for sending, or the editor is unavailable.
    Prepare,
    /// The files a forward was to carry could not be read.
    ForwardAttachments,
}

impl ComposerNotice {
    pub(crate) fn text(self) -> &'static str {
        match self {
            Self::Prepare => crate::l10n::compose_prepare_error(),
            Self::ForwardAttachments => crate::l10n::compose_forward_attachments_failed(),
        }
    }
}
