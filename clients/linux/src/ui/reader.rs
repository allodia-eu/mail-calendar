//! Who a reading view is *for*, and where a draft is being written.
//!
//! The shell used to have one of each: the reading pane and the composer that replaces it. A
//! desktop opens a message, and a reply to it, in windows of their own beside the mailbox
//! (`docs/reading-window.md`), so nothing that acts on "the open message" can mean the pane by
//! default any more. Every such input names its reader and the model resolves it to that reader's
//! state, which is what keeps two readers from acting on each other's mail.

/// One viewer of a message body, mirroring the core's own `ReaderId`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) enum ReadingSource {
    /// The reading pane in the mailbox window.
    Pane,
    /// One detached reading window, by the id the core holds its body under. A variant rather
    /// than a reserved string, so no host mistake can empty the pane by closing a window.
    Window(String),
}

impl ReadingSource {
    /// The id a window's slot is named by.
    ///
    /// Derived from the message rather than minted, which is what makes a second double-click on
    /// a row reach the window that is already up instead of stacking another on it. The pair is
    /// unique: a provider key is unique within its account.
    pub(crate) fn window_id(account: &str, key: &str) -> String {
        format!("{account}/{key}")
    }

    /// The window this names, or `None` for the pane.
    pub(crate) fn window(&self) -> Option<&str> {
        match self {
            Self::Pane => None,
            Self::Window(id) => Some(id),
        }
    }
}

/// Where a draft is being written.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum ComposerHost {
    /// The composer that replaces the reading pane, which is where every draft raised in the
    /// mailbox window goes (`docs/capabilities.md`, "Composing keeps the mailbox live").
    Pane,
    /// One detached composer window, by the id minted when the draft was raised. A minted id
    /// rather than the message being answered: two replies to one message are two drafts.
    Window(u64),
}

#[cfg(test)]
mod tests {
    use super::ReadingSource;

    #[test]
    fn a_windows_id_is_its_message_and_the_pane_is_not_a_string() {
        let source = ReadingSource::Window(ReadingSource::window_id("account", "key"));

        assert_eq!(source.window(), Some("account/key"));
        assert_eq!(ReadingSource::Pane.window(), None);
        // Two accounts can hold the same provider key, and each deserves its own window.
        assert_ne!(
            ReadingSource::window_id("account", "key"),
            ReadingSource::window_id("other", "key")
        );
    }
}
