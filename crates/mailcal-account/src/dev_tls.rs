//! Dev-only TLS trust extension; compiled **only** into non-production builds: a debug build
//! (via `debug_assertions`), or a release build with the `dev-harness` Cargo feature (the Android
//! dev loop, whose core is `--release`). A production release **without** that feature excludes it
//! entirely. The gate is `#[cfg(any(debug_assertions, feature = "dev-harness"))]` (see `lib.rs`).
//!
//! It loads an extra CA certificate named by the `MAILCAL_EXTRA_CA` environment variable and
//! folds it into the account's [`TlsPolicy`](engine_tls::TlsPolicy) as a custom root, so a debug
//! build can drive a local test server (the Stalwart harness, whose IMAP listener serves a
//! self-signed cert) over TLS. This only **adds** a trust anchor: standard rustls chain and
//! hostname verification still run, so it never accepts an invalid certificate and never skips a
//! check; it is not `danger_accept_invalid_certs`. Real accounts keep verifying against bundled
//! and OS roots exactly as before. See `docker/stalwart/README.md`.

use engine_tls::CertificateDer;
use rustls_pki_types::pem::PemObject;

/// The extra trust-anchor certificates to add for a dev build: every certificate in the PEM file
/// named by `MAILCAL_EXTRA_CA`. Returns an empty vector when the variable is unset or the file is
/// missing/unparsable: a dev convenience must never break the normal (Mozilla-roots) path.
pub(crate) fn extra_ca_anchors() -> Vec<CertificateDer<'static>> {
    let Some(path) = std::env::var_os("MAILCAL_EXTRA_CA") else {
        return Vec::new();
    };
    let Ok(pem) = std::fs::read(&path) else {
        return Vec::new();
    };
    CertificateDer::pem_slice_iter(&pem)
        .filter_map(Result::ok)
        .collect()
}
