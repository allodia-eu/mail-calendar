//! Linux host half of discoverable JMAP OAuth: loopback browser handoff and secure storage.

use std::sync::{Arc, atomic::AtomicBool};

use mailcal_bindings::MailcalApp;
use url::Url;

use super::oauth_loopback::{self, OAuthLoopback};
use crate::l10n;

pub(crate) struct JmapPrepared {
    pub(super) authorization_url: String,
    pub(super) pending: String,
    pub(super) expected_state: String,
    pub(super) loopback: OAuthLoopback,
}

pub(crate) struct JmapReauthPrepared {
    pub(super) account_id: String,
    pub(super) authorization_url: String,
    pub(super) pending: String,
    pub(super) expected_state: String,
    pub(super) redirect_uri: String,
}

#[derive(Debug)]
pub(crate) enum JmapOutcome {
    Added(String),
    Cancelled,
    Failed,
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum JmapReauthOutcome {
    Reauthenticated,
    Cancelled,
    Failed,
}

pub(super) fn prepare(
    app: &Arc<MailcalApp>,
    loopback: OAuthLoopback,
    email: String,
    server_url: String,
) -> Result<JmapPrepared, String> {
    // Dynamic registration must receive the exact redirect URI, so bind the ephemeral port before
    // asking the shared core to discover and register this install.
    let start = app
        .begin_jmap_login(
            email,
            (!server_url.trim().is_empty()).then_some(server_url),
            loopback.redirect_uri(),
        )
        .map_err(|_| l10n::setup_jmap_signin_failed().to_owned())?;
    let expected_state = authorization_state(&start.authorization_url)
        .ok_or_else(|| l10n::setup_jmap_signin_failed().to_owned())?;
    Ok(JmapPrepared {
        authorization_url: start.authorization_url,
        pending: start.pending,
        expected_state,
        loopback,
    })
}

pub(super) fn launch_browser(authorization_url: &str, on_error: impl FnOnce() + 'static) {
    oauth_loopback::launch_browser(authorization_url, on_error);
}

pub(super) fn wait(
    loopback: OAuthLoopback,
    cancel: &AtomicBool,
    expected_state: &str,
) -> CallbackOutcome {
    loopback.wait_for_state(
        cancel,
        expected_state,
        l10n::setup_jmap_signin_failed(),
        l10n::setup_jmap_signin_failed(),
    )
}

fn authorization_state(authorization_url: &str) -> Option<String> {
    Url::parse(authorization_url)
        .ok()?
        .query_pairs()
        .find_map(|(key, value)| (key == "state").then(|| value.into_owned()))
}

fn authorization_redirect_uri(authorization_url: &str) -> Option<String> {
    let redirect = Url::parse(authorization_url)
        .ok()?
        .query_pairs()
        .find_map(|(key, value)| (key == "redirect_uri").then(|| value.into_owned()))?;
    let uri = Url::parse(&redirect).ok()?;
    (uri.scheme() == "http"
        && uri.host_str() == Some("127.0.0.1")
        && uri.port().is_some()
        && uri.path() == "/"
        && uri.query().is_none()
        && uri.fragment().is_none())
    .then_some(redirect)
}

pub(super) fn prepare_reauth(
    app: &Arc<MailcalApp>,
    account_id: String,
) -> Result<JmapReauthPrepared, String> {
    let start = app
        .begin_jmap_reauth(account_id.clone())
        .map_err(|_| l10n::signin_expired_failed().to_owned())?;
    let expected_state = authorization_state(&start.authorization_url)
        .ok_or_else(|| l10n::signin_expired_failed().to_owned())?;
    let redirect_uri = authorization_redirect_uri(&start.authorization_url)
        .ok_or_else(|| l10n::signin_expired_failed().to_owned())?;
    Ok(JmapReauthPrepared {
        account_id,
        authorization_url: start.authorization_url,
        pending: start.pending,
        expected_state,
        redirect_uri,
    })
}

pub(super) fn complete(
    app: &Arc<MailcalApp>,
    pending: String,
    callback_url: String,
) -> JmapOutcome {
    let Ok(config) = app.complete_jmap_login(pending, callback_url) else {
        return JmapOutcome::Failed;
    };
    // `add_account` writes the grant through the host's `AccountCredentialStore` and rolls the
    // add back itself when that write fails, so a failure here has left nothing behind.
    let Ok(account) = app.add_account(config) else {
        return JmapOutcome::Failed;
    };
    JmapOutcome::Added(account.id)
}

pub(super) fn complete_reauth(
    app: &Arc<MailcalApp>,
    account_id: String,
    pending: String,
    callback_url: String,
) -> JmapReauthOutcome {
    match app.complete_jmap_reauth(account_id, pending, callback_url) {
        Ok(()) => JmapReauthOutcome::Reauthenticated,
        Err(_) => JmapReauthOutcome::Failed,
    }
}

pub(super) use super::oauth_loopback::CallbackOutcome;

#[cfg(test)]
mod tests {
    use super::{authorization_redirect_uri, authorization_state};

    #[test]
    fn authorization_state_is_taken_from_the_url_without_logging_the_pending_handle() {
        assert_eq!(
            authorization_state("https://auth.example.test/?client_id=x&state=expected&scope=mail")
                .as_deref(),
            Some("expected")
        );
        assert!(authorization_state("https://auth.example.test/?client_id=x").is_none());
    }

    #[test]
    fn a_reauthentication_names_the_persisted_loopback_listener_to_bind() {
        assert_eq!(
            authorization_redirect_uri(
                "https://auth.example.test/?state=expected&redirect_uri=http%3A%2F%2F127.0.0.1%3A32145%2F"
            )
            .as_deref(),
            Some("http://127.0.0.1:32145/")
        );
        assert!(
            authorization_redirect_uri(
                "https://auth.example.test/?state=expected&redirect_uri=https%3A%2F%2Fevil.example%2F"
            )
            .is_none(),
            "the host must never bind or trust a non-loopback redirect"
        );
    }
}
