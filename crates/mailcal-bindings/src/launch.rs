//! What a host shows while the core it asked for is still opening.

/// How long, in milliseconds, a host lets [`crate::MailcalApp::new_accounts`] run before its launch
/// view says the mailbox is being opened (`docs/boot-sequence.md`). Until then the window is a
/// blank page: a normal open is over well inside this, and a label raised and removed within it
/// reads as flicker rather than as a fast launch.
#[uniffi::export]
#[must_use]
pub fn launch_status_after_ms() -> u32 {
    500
}
