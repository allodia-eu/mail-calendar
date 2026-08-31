//! Account-level TLS policy selection.
//!
//! Native clients use the Firefox-style trust policy (bundled Mozilla roots plus the OS store).
//! Dev-harness builds may add the local Stalwart CA as an explicit custom root without switching
//! to Android's platform verifier path.

use engine_tls::{CertificateDer, TlsClientConfig, TlsError, TlsPolicy, client_config};

/// Builds the shared TLS config for one account's providers.
pub(crate) fn account_tls() -> Result<TlsClientConfig, TlsError> {
    let custom = custom_roots();
    let policy = if custom.is_empty() {
        TlsPolicy::bundled_and_system()
    } else {
        TlsPolicy::roots(true, true, custom)
    };
    client_config(&policy)
}

#[cfg(any(debug_assertions, feature = "dev-harness"))]
fn custom_roots() -> Vec<CertificateDer<'static>> {
    crate::dev_tls::extra_ca_anchors()
}

#[cfg(not(any(debug_assertions, feature = "dev-harness")))]
fn custom_roots() -> Vec<CertificateDer<'static>> {
    Vec::new()
}
