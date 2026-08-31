//! Screenshot mode: the seeded in-memory dataset instead of the developer's real accounts.
//!
//! `MAILCAL_SHOWCASE` brings the app up on the showcase dataset (two fictional accounts, seeded
//! sample content) so store and listing screenshots need no personal mail. A listing wants a set
//! per language, so the variable doubles as the language switch: `MAILCAL_SHOWCASE=nl` gives Dutch
//! chrome *and* Dutch sample mail in one launch, applied for the session only so a capture run
//! never rewrites the developer's stored preference. `MAILCAL_SHOWCASE_SCREEN` then picks which
//! screen the run drives to. The Linux twin of the Apple client's `ShowcaseMode.swift`, Android's
//! `ShowcaseMode.kt` and Windows' `Services/ShowcaseMode.cs`.
//!
//! The whole file is compiled out of a release build, so a shipped binary cannot have its mailbox
//! replaced by a stray environment variable: the twin of Apple's and Windows' `#if DEBUG` and
//! Android's `FLAG_DEBUGGABLE` check.

#![cfg(any(debug_assertions, feature = "dev-harness"))]

use mailcal_bindings::{
    ShowcaseInvitation, ShowcaseLocale, ShowcaseReply, showcase_invitation,
    showcase_locale_for_language, showcase_reply,
};

use crate::l10n;

/// The values that mean "off", so `MAILCAL_SHOWCASE=0` reads as unset rather than as a language.
const OFF: [&str; 5] = ["", "0", "false", "no", "off"];

/// Which screen a screenshot run drives to once the mailbox has loaded.
///
/// The flag spellings are the cross-client contract: `scripts/dev/showcase.sh` passes the same
/// names to every client; so they are matched literally.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ShowcaseScreen {
    /// The message list. Linux has a reading pane, so the first message opens into it.
    List,
    /// The reply composer for the showcase's designated message, pre-filled with sample text.
    Reply,
    /// The settings surface.
    Settings,
    /// The settings surface, opened on the Signatures category.
    Signatures,
    /// The add-an-account form.
    AddAccount,
    /// The calendar, opened on today.
    Calendar,
    /// The meeting-invitation card: the seeded inbox's iMIP `REQUEST`, opened and left there.
    Invitation,
}

/// Reads a screen name, refusing one this client cannot reach.
///
/// **Deliberately not the other clients' "unknown falls back to the list".** That fallback is safe
/// where every offered screen exists; a name this client has no surface for would otherwise
/// photograph the *inbox* and file it under that screen's name; a clean, well-lit,
/// correctly-sized capture of the wrong screen, which nothing downstream can detect. Refusing makes
/// the run fail at the launch instead. `check-showcase-flag.sh` holds the arms below against the
/// list `scripts/dev/showcase.sh` offers Linux.
pub(crate) fn parse_screen(raw: Option<&str>) -> Result<ShowcaseScreen, String> {
    match raw
        .map(|value| value.trim().to_ascii_lowercase())
        .as_deref()
    {
        None | Some("" | "list") => Ok(ShowcaseScreen::List),
        Some("reply") => Ok(ShowcaseScreen::Reply),
        Some("settings") => Ok(ShowcaseScreen::Settings),
        Some("signatures") => Ok(ShowcaseScreen::Signatures),
        Some("add-account") => Ok(ShowcaseScreen::AddAccount),
        Some("calendar") => Ok(ShowcaseScreen::Calendar),
        Some("invitation") => Ok(ShowcaseScreen::Invitation),
        Some(other) => Err(other.to_owned()),
    }
}

/// The trimmed, lower-cased `MAILCAL_SHOWCASE`, or `None` when it is unset or names an "off" value.
fn raw() -> Option<String> {
    let value = std::env::var("MAILCAL_SHOWCASE")
        .ok()?
        .trim()
        .to_ascii_lowercase();
    (!OFF.contains(&value.as_str())).then_some(value)
}

/// Whether screenshot mode is on.
pub(crate) fn is_on() -> bool {
    raw().is_some()
}

/// The language the variable pins the whole app to, or `None` when it names none
/// (`MAILCAL_SHOWCASE=1`) and the app's own language choice stands.
///
/// Read off the emitted catalog list rather than a list kept here, so a language added to
/// `messages/` is offered without editing this file.
pub(crate) fn language_override() -> Option<String> {
    raw().filter(|value| l10n::LOCALES.contains(&value.as_str()))
}

/// The locale to seed the showcase mailbox and calendar in: the pinned language when the variable
/// names one, else whatever the chrome resolved to. Dutch chrome over English mail reads as a
/// broken screenshot, so the sample content always follows the UI. The code → locale mapping lives
/// in the core, so every client seeds the same content for the same language.
pub(crate) fn seed_locale() -> ShowcaseLocale {
    showcase_locale_for_language(
        language_override().unwrap_or_else(|| l10n::active_locale().to_owned()),
    )
}

/// The screen this run drives to, or the unrecognised name it was asked for.
pub(crate) fn screen() -> Result<ShowcaseScreen, String> {
    parse_screen(std::env::var("MAILCAL_SHOWCASE_SCREEN").ok().as_deref())
}

/// The message the reply screenshot replies to, and the sample body to seed the composer with.
pub(crate) fn reply_target() -> ShowcaseReply {
    showcase_reply(seed_locale())
}

/// The message the invitation screenshot opens.
///
/// Takes no locale, and the core says why: the account and key are identical in every seed, and
/// only the meeting's words are translated.
pub(crate) fn invitation_target() -> ShowcaseInvitation {
    showcase_invitation()
}

/// The sample reply text for the showcase's designated message, else `None`: every other message,
/// and every non-showcase run, keeps the normal empty composer. Plain text; see
/// `docs/composer-security.md` Gate 11.
pub(crate) fn reply_text(account: &str, key: &str) -> Option<String> {
    if !is_on() {
        return None;
    }
    let reply = reply_target();
    (account == reply.account && key == reply.message_key).then_some(reply.text)
}

#[cfg(test)]
mod tests {
    use super::{ShowcaseScreen, parse_screen};

    #[test]
    fn the_contract_spellings_are_matched_literally() {
        assert_eq!(parse_screen(Some("reply")), Ok(ShowcaseScreen::Reply));
        assert_eq!(
            parse_screen(Some("  Calendar ")),
            Ok(ShowcaseScreen::Calendar)
        );
        assert_eq!(
            parse_screen(Some("add-account")),
            Ok(ShowcaseScreen::AddAccount)
        );
        assert_eq!(
            parse_screen(Some("signatures")),
            Ok(ShowcaseScreen::Signatures)
        );
        // Not a spelling we accept, on any client.
        assert_eq!(
            parse_screen(Some("addaccount")),
            Err("addaccount".to_owned())
        );
    }

    #[test]
    fn an_unset_screen_is_the_list_and_an_unreachable_one_is_refused() {
        assert_eq!(parse_screen(None), Ok(ShowcaseScreen::List));
        assert_eq!(parse_screen(Some("")), Ok(ShowcaseScreen::List));
        assert_eq!(
            parse_screen(Some("invitation")),
            Ok(ShowcaseScreen::Invitation)
        );
        // A name no client has a surface for. Refused rather than silently answered with a
        // photograph of the inbox, which every later check in the capture run would pass.
        assert_eq!(parse_screen(Some("agenda")), Err("agenda".to_owned()));
    }
}
