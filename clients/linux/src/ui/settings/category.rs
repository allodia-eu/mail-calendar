//! What each Settings category is called: its stack name, its title and its glyph.

use super::{Category, icons};
use crate::l10n;

impl Category {
    pub(super) const fn name(self) -> &'static str {
        match self {
            Self::Allodia => "allodia",
            Self::General => "general",
            Self::Calendar => "calendar",
            Self::Reading => "reading",
            Self::Composing => "composing",
            Self::Signatures => "signatures",
            Self::Notifications => "notifications",
            Self::Privacy => "privacy",
            Self::Accounts => "accounts",
            Self::Advanced => "advanced",
            Self::Diagnostics => "diagnostics",
            Self::About => "about",
        }
    }

    pub(super) fn title(self) -> &'static str {
        match self {
            // The category and the group inside it are the same thing named twice otherwise, and
            // the product's own name belongs in one string.
            Self::Allodia => l10n::settings_allodia_heading(),
            Self::General => l10n::settings_category_general(),
            Self::Calendar => l10n::settings_category_calendar(),
            Self::Reading => l10n::settings_category_reading(),
            Self::Composing => l10n::settings_category_composing(),
            Self::Signatures => l10n::settings_category_signatures(),
            Self::Notifications => l10n::settings_category_notifications(),
            Self::Privacy => l10n::settings_category_privacy(),
            Self::Accounts => l10n::settings_category_accounts(),
            Self::Advanced => l10n::settings_category_advanced(),
            Self::Diagnostics => l10n::settings_category_diagnostics(),
            Self::About => l10n::settings_category_about(),
        }
    }

    pub(super) const fn icon(self) -> &'static str {
        match self {
            Self::Allodia => icons::SETTINGS_ALLODIA,
            Self::General => icons::SETTINGS_GENERAL,
            Self::Calendar => icons::SETTINGS_CALENDAR,
            Self::Reading => icons::SETTINGS_READING,
            Self::Composing => icons::SETTINGS_COMPOSING,
            Self::Signatures => icons::SETTINGS_SIGNATURES,
            Self::Notifications => icons::SETTINGS_NOTIFICATIONS,
            Self::Privacy => icons::SETTINGS_PRIVACY,
            Self::Accounts => icons::SETTINGS_ACCOUNTS,
            Self::Advanced => icons::SETTINGS_ADVANCED,
            Self::Diagnostics => icons::SETTINGS_DIAGNOSTICS,
            Self::About => icons::SETTINGS_ABOUT,
        }
    }
}
