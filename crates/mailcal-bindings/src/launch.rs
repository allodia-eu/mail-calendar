//! What a host shows while the core it asked for is still opening.

/// How long, in milliseconds, a host lets [`crate::MailcalApp::new_accounts`] run before its launch
/// view says the mailbox is being opened (`docs/boot-sequence.md`). Until then the window is a
/// blank page: a normal open takes up to about a second on a desktop, and a label raised and
/// removed within it reads as flicker rather than as a fast launch. A store migration after an
/// update takes longer, and that is the wait this is for.
#[uniffi::export]
#[must_use]
pub fn launch_status_after_ms() -> u32 {
    2000
}
