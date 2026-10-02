//! Linux host half of IMAP OAuth sign-in: the loopback browser handoff and the account write.
//!
//! The twin of [`super::jmap`], and deliberately as thin: the loopback listener, the browser
//! launch and the redirect wait are all [`super::oauth_loopback`]'s, which is protocol-neutral
//! and shared. What is here is the pair of core calls that differ, and the copy shown when one
//! of them fails.

use std::sync::{Arc, atomic::AtomicBool};

use mailcal_bindings::{ImapLoginRequest, MailcalApp};

use super::{
    oauth_loopback::{self, OAuthLoopback},
    setup_model::ImapForm,
};
use crate::l10n;

/// Everything the browser half needs, once the core has discovered and registered.
pub(crate) struct ImapPrepared {
    pub(super) authorization_url: String,
    pub(super) pending: String,
    pub(super) expected_state: String,
    pub(super) loopback: OAuthLoopback,
}

#[derive(Debug)]
pub(crate) enum ImapOutcome {
    Added(String),
    Cancelled,
    Failed,
}

/// The request the pre-flight and the sign-in both describe the account with.
///
/// One conversion, used by both, so the two cannot come to different conclusions about the
/// same account: a pre-flight that probed a different server from the one the sign-in
/// registers against would offer a button that fails at the provider.
pub(super) fn login_request(form: &ImapForm) -> ImapLoginRequest {
    ImapLoginRequest {
        email: form.email.clone(),
        imap_host: form.imap_host.clone(),
        smtp_host: (!form.smtp_host.trim().is_empty()).then(|| form.smtp_host.clone()),
        caldav_base_url: (!form.caldav_url.trim().is_empty()).then(|| form.caldav_url.clone()),
        imap_security: Some(form.imap_security),
        smtp_security: Some(form.smtp_security),
        oauth_issuer: form.oauth_issuer.clone(),
    }
}

/// Discovers, registers and builds the authorization request.
///
/// The loopback port is bound **before** this runs, because dynamic registration must be given
/// the exact redirect URI the browser will come back to.
pub(super) fn prepare(
    app: &Arc<MailcalApp>,
    loopback: OAuthLoopback,
    request: ImapLoginRequest,
) -> Result<ImapPrepared, String> {
    let start = app
        .begin_imap_login(request, loopback.redirect_uri())
        .map_err(|_| l10n::setup_imap_signin_failed().to_owned())?;
    let expected_state = authorization_state(&start.authorization_url)
        .ok_or_else(|| l10n::setup_imap_signin_failed().to_owned())?;
    Ok(ImapPrepared {
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
        l10n::setup_imap_signin_failed(),
        l10n::setup_imap_signin_failed(),
    )
}

/// Exchanges the code and adds the account.
pub(super) fn complete(
    app: &Arc<MailcalApp>,
    pending: String,
    callback_url: String,
) -> ImapOutcome {
    let Ok(config) = app.complete_imap_login(pending, callback_url) else {
        return ImapOutcome::Failed;
    };
    // `add_account` writes the grant through the host's `AccountCredentialStore` and rolls the
    // add back itself when that write fails, so a failure here has left nothing behind.
    let Ok(account) = app.add_account(config) else {
        return ImapOutcome::Failed;
    };
    ImapOutcome::Added(account.id)
}

/// The `state` the authorization request minted, which the redirect must echo.
///
/// Read off the URL rather than returned beside it, exactly as the JMAP flow does: the
/// pending handle carries the PKCE verifier and must not be picked apart to find one field.
fn authorization_state(authorization_url: &str) -> Option<String> {
    url::Url::parse(authorization_url)
        .ok()?
        .query_pairs()
        .find_map(|(key, value)| (key == "state").then(|| value.into_owned()))
}

pub(super) use super::oauth_loopback::CallbackOutcome;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::setup_model::{DetectedServer, ImapSignIn};

    fn form() -> ImapForm {
        ImapForm {
            email: "alice@example.com".to_owned(),
            imap_host: "imap.example.com".to_owned(),
            smtp_host: "smtp.example.com".to_owned(),
            caldav_url: String::new(),
            imap_security: mailcal_bindings::ConnectionSecurity::StartTls,
            smtp_security: mailcal_bindings::ConnectionSecurity::StartTls,
            trusted: true,
            incoming: DetectedServer {
                protocol: "IMAP".to_owned(),
                hostname: "imap.example.com".to_owned(),
                port: 143,
                security: "STARTTLS".to_owned(),
            },
            outgoing: None,
            oauth_issuer: Some("https://login.example.com".to_owned()),
            sign_in: ImapSignIn::Checking,
        }
    }

    #[test]
    fn the_request_carries_the_detected_transports_and_the_named_issuer() {
        // Both are things the core cannot re-derive: the security decides which port is
        // probed and then dialled, and the issuer is what the provider said about itself, so
        // dropping either sends the flow at a different server from the detected one.
        let request = login_request(&form());
        assert_eq!(request.imap_host, "imap.example.com");
        assert_eq!(
            request.imap_security,
            Some(mailcal_bindings::ConnectionSecurity::StartTls)
        );
        assert_eq!(
            request.oauth_issuer.as_deref(),
            Some("https://login.example.com")
        );
    }

    #[test]
    fn an_empty_optional_field_is_absent_rather_than_blank() {
        // A blank CalDAV URL must not reach the core as `Some("")`: the account would carry a
        // `[caldav]` section pointing at nothing, and the calendar would fail on every sync.
        let request = login_request(&form());
        assert!(request.caldav_base_url.is_none());
        assert_eq!(request.smtp_host.as_deref(), Some("smtp.example.com"));
    }

    #[test]
    fn the_state_the_redirect_must_echo_is_read_off_the_authorization_url() {
        assert_eq!(
            authorization_state("https://auth.example.test/?client_id=x&state=expected").as_deref(),
            Some("expected")
        );
        assert!(authorization_state("https://auth.example.test/?client_id=x").is_none());
    }
}
