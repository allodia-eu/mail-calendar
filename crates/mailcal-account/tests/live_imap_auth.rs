//! Gated live check of the setup decision, against the local Stalwart harness.
//!
//! The offline suite pins the arithmetic around the decision; this is the one thing it cannot
//! show, which is that the probe reaches a real server, reads a real capability line, and the
//! answer that comes back is the one the setup screen would draw.
//!
//! Skips with no `MAILCAL_HARNESS_IMAP`, so `cargo test --workspace` stays green with no Docker.
//! Start the harness with `scripts/dev/harness.sh up`, then:
//!
//! ```sh
//! export MAILCAL_EXTRA_CA="$PWD/docker/stalwart/tls/harness-ca.pem"
//! export MAILCAL_HARNESS_IMAP=localhost:12993
//! ```
//!
//! Both variables matter, and getting either wrong looks identical to the code being broken. The
//! probe dials over the account's **verifying** connector, so without the harness CA the TLS
//! handshake fails and the decision fail-softs to `Password`, which is exactly what a real server
//! that refuses OAuth produces. And the harness certificate's only SAN is `localhost`, so
//! `127.0.0.1` fails the same way for a different reason.

use mailcal_account::{ConnectionSecurity, ImapAuth, ImapAuthQuery, decide_imap_auth};

/// The harness address, or `None` when this run is not gated on.
fn harness() -> Option<String> {
    std::env::var("MAILCAL_HARNESS_IMAP")
        .ok()
        .filter(|addr| !addr.is_empty())
}

#[tokio::test]
async fn the_harness_offers_oauth_but_names_no_authorization_server_we_can_reach() {
    let Some(addr) = harness() else {
        eprintln!("skipping: MAILCAL_HARNESS_IMAP unset");
        return;
    };
    let query = ImapAuthQuery {
        imap_host: addr,
        imap_security: ConnectionSecurity::ImplicitTls,
        email: "alice@mail.test.local".to_owned(),
        autoconfig_issuer: None,
    };

    // Stalwart advertises `AUTH=PLAIN AUTH=OAUTHBEARER AUTH=XOAUTH2`, so the probe must see an
    // OAuth mechanism *and* a password. What it cannot find is a reachable authorization server:
    // Stalwart derives its issuer from its configured hostname (`https://mail.test.local`) while
    // the harness maps only a loopback HTTP port, so the metadata that issuer implies does not
    // resolve from here. `RegistrationNeeded` is therefore the truthful answer, and asserting it
    // proves the whole chain ran: the dial, the capability read, the OAuth branch, and the issuer
    // search giving up honestly rather than offering a button that would dead-end.
    match decide_imap_auth(&query).await {
        ImapAuth::RegistrationNeeded {
            password_also_works,
        } => assert!(
            password_also_works,
            "the harness takes AUTH=PLAIN, so the password route must stay open"
        ),
        // A harness reconfigured to publish a reachable issuer is the better outcome, not a
        // failure: it means the sign-in can be driven by hand against it.
        ImapAuth::SignIn {
            password_also_works,
            ..
        } => assert!(password_also_works),
        ImapAuth::Password => panic!(
            "the harness advertises AUTH=OAUTHBEARER; reporting a password-only server means the \
             probe never reached it"
        ),
    }
}
