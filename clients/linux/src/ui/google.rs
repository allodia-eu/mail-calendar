//! Google Desktop OAuth host adapter: configuration plus secure completion.

use std::sync::{Arc, atomic::AtomicBool};

use mailcal_bindings::{GoogleLoginStart, MailcalApp, begin_google_login};

use super::oauth_loopback::{self, CallbackOutcome, OAuthLoopback};
use crate::l10n;

#[derive(Debug)]
pub(crate) enum GoogleOutcome {
    Added(String),
    Cancelled,
    Failed(String),
}

pub(super) fn begin(login_hint: String) -> Result<(OAuthLoopback, GoogleLoginStart), String> {
    let loopback =
        OAuthLoopback::bind().map_err(|_| l10n::setup_google_browser_failed().to_owned())?;
    let start = begin_google_login(
        loopback.redirect_uri(),
        (!login_hint.trim().is_empty()).then_some(login_hint),
    )
    .map_err(|error| error.to_string())?;
    Ok((loopback, start))
}

pub(super) fn wait(loopback: OAuthLoopback, cancel: &AtomicBool) -> CallbackOutcome {
    loopback.wait(
        cancel,
        l10n::setup_google_timeout(),
        l10n::setup_google_browser_failed(),
    )
}

pub(super) fn launch_browser(authorization_url: &str, on_error: impl FnOnce(String) + 'static) {
    oauth_loopback::launch_browser(authorization_url, move || {
        on_error(l10n::setup_google_browser_failed().to_owned());
    });
}

pub(super) fn complete(
    app: &Arc<MailcalApp>,
    pending: String,
    callback_url: String,
) -> GoogleOutcome {
    match app.complete_google_login(pending, callback_url) {
        // The core writes the grant through the host's `AccountCredentialStore` and rolls the
        // add back itself when that write fails, so there is nothing for the client to persist.
        Ok(account) => GoogleOutcome::Added(account.id),
        Err(error) => GoogleOutcome::Failed(error.to_string()),
    }
}
