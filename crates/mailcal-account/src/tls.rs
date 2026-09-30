//! Account-level TLS policy selection.
//!
//! Native clients use the Firefox-style trust policy (bundled Mozilla roots plus the OS store).
//! Dev-harness builds may add the local Stalwart CA as an explicit custom root without switching
//! to Android's platform verifier path.
//!
//! On top of that policy an account carries the certificates its owner accepted for a server
//! that could not be verified (`docs/certificate-exceptions.md`). Those never widen the policy:
//! the verifier consults them only once it has already refused, and each admits one certificate
//! on one server name.

use engine_tls::{
    CertificateDer, CertificateException, TlsClientConfig, TlsError, TlsPolicy,
    client_config_with_exceptions,
};

use crate::AccountConfig;

/// Builds the shared TLS config for one account's providers, honouring the certificates
/// its owner accepted.
pub(crate) fn account_tls(account: &AccountConfig) -> Result<TlsClientConfig, TlsError> {
    tls_with(&account.tls_exceptions())
}

/// The same config for a connection that belongs to no [`AccountConfig`] (a JMAP account
/// keeps its own), so one place decides the policy.
pub(crate) fn tls_with(exceptions: &[CertificateException]) -> Result<TlsClientConfig, TlsError> {
    client_config_with_exceptions(&setup_trust_policy(), exceptions)
}

/// The trust policy, with no exceptions, for the HTTPS setup makes before an account exists:
/// detection's autoconfig fetches, and the metadata that decides where a mail account signs in.
///
/// The same policy the account's own connections get, so a debug build that trusts the harness's
/// IMAP listener also trusts the HTTPS front that names its authorization server. Anything
/// fetched past that point (registration, the code exchange, a refresh) is not covered, which the
/// harness never needs: those endpoints are the sign-in server's own plain loopback HTTP.
#[must_use]
pub fn setup_trust_policy() -> TlsPolicy {
    let custom = custom_roots();
    if custom.is_empty() {
        TlsPolicy::bundled_and_system()
    } else {
        TlsPolicy::roots(true, true, custom)
    }
}

#[cfg(any(debug_assertions, feature = "dev-harness"))]
fn custom_roots() -> Vec<CertificateDer<'static>> {
    crate::dev_tls::extra_ca_anchors()
}

#[cfg(not(any(debug_assertions, feature = "dev-harness")))]
fn custom_roots() -> Vec<CertificateDer<'static>> {
    Vec::new()
}
