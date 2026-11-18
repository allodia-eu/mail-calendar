//! The importance banner above an open message's body (`docs/importance.md`).
//!
//! Whether there is one, and which, is the core's `ReadingSnapshot::importance_banner`; this only
//! words and draws it. It has no button: there is nothing to do about a sender's importance.

use mailcal_bindings::MessageImportance;

use crate::l10n;

pub(super) fn banner() -> adw::Banner {
    adw::Banner::new("")
}

/// Shows the banner the core asked for, or hides it for `None`.
pub(super) fn show(banner: &adw::Banner, importance: Option<MessageImportance>) {
    let title = match importance {
        Some(MessageImportance::High) => l10n::reading_importance_high(),
        Some(MessageImportance::Low) => l10n::reading_importance_low(),
        Some(MessageImportance::Normal) | None => {
            banner.set_revealed(false);
            return;
        }
    };
    banner.set_title(title);
    banner.set_revealed(true);
}
