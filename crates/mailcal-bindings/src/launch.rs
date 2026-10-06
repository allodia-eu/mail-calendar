//! What a host shows while the core it asked for is still opening.

use std::time::Duration;

/// How long a host lets [`crate::MailcalApp::new_accounts`] run before its launch view says the
/// mailbox is being opened (`docs/boot-sequence.md`). Until then the window is a blank page: a
/// normal open is over well inside this, and a label raised and removed within it reads as
/// flicker rather than as a fast launch.
const LAUNCH_STATUS_AFTER: Duration = Duration::from_millis(500);

/// [`LAUNCH_STATUS_AFTER`] in milliseconds, for a host to time its launch view by.
#[uniffi::export]
#[must_use]
pub fn launch_status_after_ms() -> u32 {
    u32::try_from(LAUNCH_STATUS_AFTER.as_millis()).unwrap_or(u32::MAX)
}
