//! `MAILCAL_FAKE_SERVER_CHANGE`: a staged answer to looking an account's servers up again.
//!
//! The paused use (`docs/account-autodetect.md` rule 14) is the one state no harness can produce:
//! the question names two sites, and a loopback server has no site the Public Suffix List can
//! place, so a real lookup against it never asks. The value names the server the lookup pretends
//! to have found, and substitutes **only** that answer. Everything around it is real: the stored
//! server has to have stopped answering for the lookup to run at all, the core pauses the use,
//! the client draws the question, and allowing it connects to the named server before anything
//! is stored. A hook that staged the pause itself would be lifted by the next dial that connects.
//!
//! Debug builds only, like the harness CA trust, so no release binary can be talked into pausing
//! a calendar whose server never moved.

use crate::AccountEndpoints;

/// The change `raw` stages for an account storing `stored`: every calendar and address-book
/// server it stores moves to the URL `raw` names. A site is the registrable domain, or the host
/// itself where the Public Suffix List cannot place one. `None` when `raw` names no server or the
/// account stores none.
#[cfg(any(debug_assertions, test))]
pub(super) fn staged_change(
    stored: &AccountEndpoints,
    raw: &str,
) -> Option<mailcal_app::ServerChange> {
    let proposed = raw.trim();
    let previous = stored
        .caldav_url
        .as_deref()
        .or(stored.carddav_url.as_deref())?;
    Some(mailcal_app::ServerChange {
        previous_site: site(previous)?,
        proposed_site: site(proposed)?,
        calendar_url: stored.caldav_url.is_some().then(|| proposed.to_owned()),
        contacts_url: stored.carddav_url.is_some().then(|| proposed.to_owned()),
    })
}

#[cfg(any(debug_assertions, test))]
fn site(url: &str) -> Option<String> {
    mailcal_autodetect::site_of(url)
        .or_else(|| url::Url::parse(url).ok()?.host_str().map(str::to_owned))
}

/// The change `MAILCAL_FAKE_SERVER_CHANGE` stages for an account storing `stored`, if it is set.
#[cfg(debug_assertions)]
pub(super) fn pretended(stored: &AccountEndpoints) -> Option<mailcal_app::ServerChange> {
    let raw = std::env::var("MAILCAL_FAKE_SERVER_CHANGE").ok()?;
    let staged = staged_change(stored, &raw);
    // Warned on both paths: this pauses a calendar the person relies on, so a log of the run must
    // say it was in force, and a value that silently did nothing proves the opposite of a test.
    match &staged {
        Some(_) => log::warn!(
            "rediscovery: MAILCAL_FAKE_SERVER_CHANGE is set; pretending the lookup found the \
             account's servers on another site"
        ),
        None => log::warn!(
            "rediscovery: MAILCAL_FAKE_SERVER_CHANGE names no server URL; looking the servers up \
             for real"
        ),
    }
    staged
}

#[cfg(not(debug_assertions))]
pub(super) fn pretended(_stored: &AccountEndpoints) -> Option<mailcal_app::ServerChange> {
    None
}
