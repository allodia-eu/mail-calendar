//! Microsoft 365 OAuth host adapter: configuration plus secure completion.
//!
//! The shared core owns the state machine (PKCE, exchange, refresh, storage); this file owns
//! only the browser half. Like the Google Desktop client, Linux captures the redirect on a
//! bounded loopback listener rather than a custom scheme; the app claims no URI scheme on this
//! platform, and `http://127.0.0.1` is registered as a redirect on the Azure app registration
//! (`docs/provider-oauth.md` rule 7).

use std::sync::{Arc, atomic::AtomicBool};

use mailcal_bindings::{MailcalApp, MicrosoftLoginStart, begin_microsoft_login};

use super::oauth_loopback::{self, CallbackOutcome, OAuthLoopback};
use crate::l10n;

/// Work and personal Microsoft accounts alike.
const MICROSOFT_TENANT: &str = "common";

#[derive(Debug)]
pub(crate) enum MicrosoftOutcome {
    Added(String),
    Cancelled,
    Failed(String),
}

pub(super) fn begin(login_hint: String) -> Result<(OAuthLoopback, MicrosoftLoginStart), String> {
    let loopback =
        OAuthLoopback::bind().map_err(|_| l10n::setup_microsoft_browser_failed().to_owned())?;
    let start = begin_microsoft_login(
        Some(MICROSOFT_TENANT.to_owned()),
        loopback.redirect_uri(),
        // With the address known, Microsoft targets that account instead of a different one
        // already signed in in the browser (`docs/provider-oauth.md` rule 8).
        (!login_hint.trim().is_empty()).then_some(login_hint),
    )
    .map_err(|error| error.to_string())?;
    Ok((loopback, start))
}

pub(super) fn wait(loopback: OAuthLoopback, cancel: &AtomicBool) -> CallbackOutcome {
    loopback.wait(
        cancel,
        l10n::setup_microsoft_timeout(),
        l10n::setup_microsoft_browser_failed(),
    )
}

pub(super) fn launch_browser(authorization_url: &str, on_error: impl FnOnce(String) + 'static) {
    oauth_loopback::launch_browser(authorization_url, move || {
        on_error(l10n::setup_microsoft_browser_failed().to_owned());
    });
}

pub(super) fn complete(
    app: &Arc<MailcalApp>,
    pending: String,
    callback_url: String,
) -> MicrosoftOutcome {
    match app.complete_microsoft_login(pending, callback_url) {
        // The core writes the grant through the host's `AccountCredentialStore` and rolls the
        // add back itself when that write fails, so there is nothing for the client to persist.
        Ok(account) => MicrosoftOutcome::Added(account.id),
        // A declined consent or an org policy blocking the app arrives here; it is shown on the
        // sign-in surface rather than swallowed (`docs/provider-oauth.md` rule 9).
        Err(error) => MicrosoftOutcome::Failed(error.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use mailcal_bindings::oauth_routes;

    use super::{MICROSOFT_TENANT, begin};

    #[test]
    fn a_sign_in_targets_this_app_registration_and_its_own_loopback() {
        // The registration is injected at build time, so this asserts whichever contract this
        // build is under: a build given none refuses to start a sign-in it cannot finish.
        let Some(client_id) = oauth_routes().microsoft.then(client_id) else {
            assert!(begin("person@outlook.com".to_owned()).is_err());
            return;
        };
        let (loopback, start) = begin("person@outlook.com".to_owned()).expect("begin sign-in");
        let redirect = loopback.redirect_uri();

        assert!(redirect.starts_with("http://127.0.0.1:"));
        // The redirect the browser comes back to must be the port actually bound, since the URI
        // is baked into the authorization request before the user ever leaves the app.
        assert!(
            start.authorization_url.contains(&urlencoding_of(&redirect)),
            "{}",
            start.authorization_url
        );
        assert!(start.authorization_url.contains(&client_id));
        assert!(start.authorization_url.contains(MICROSOFT_TENANT));
        // A known address targets that account rather than one already signed in in the browser.
        assert!(start.authorization_url.contains("login_hint"));

        // Without an address, Microsoft shows its picker instead.
        let (_, start) = begin(String::new()).expect("begin sign-in");
        assert!(!start.authorization_url.contains("login_hint"));
        assert!(start.authorization_url.contains("prompt=select_account"));
    }

    fn urlencoding_of(value: &str) -> String {
        value.replace(':', "%3A").replace('/', "%2F")
    }

    /// The client id this build was given. Read from an authorization URL rather than from the
    /// core, which deliberately exposes only whether the route exists.
    fn client_id() -> String {
        let (_, start) = begin(String::new()).expect("begin sign-in");
        let url = url::Url::parse(&start.authorization_url).expect("an authorization URL");
        url.query_pairs()
            .find(|(key, _)| key == "client_id")
            .map(|(_, value)| value.into_owned())
            .expect("a client_id in the authorization URL")
    }
}
