//! Gated live check of the setup decision, against the local Stalwart harness.
//!
//! The offline suite pins the arithmetic around the decision; this is the one thing it cannot
//! show, which is that the probe reaches a real server, reads a real capability line, and the
//! answer that comes back is the one the setup screen would draw.
//!
//! Two servers, two answers. The seeded server takes a token but names no authorization server
//! anyone can reach. The sign-in server (`stalwart-oauth`) names one through its HTTPS front,
//! which a client learns from the provider's own autoconfig (`docker/stalwart/front/`).
//!
//! Each case skips when its address is unset, so `cargo test --workspace` stays green with no
//! Docker. `scripts/dev/harness.sh test` sets all three; by hand:
//!
//! ```sh
//! export MAILCAL_EXTRA_CA="$PWD/docker/stalwart/tls/harness-ca.pem"
//! export MAILCAL_HARNESS_IMAP=localhost:12993
//! export MAILCAL_HARNESS_OAUTH_IMAP=localhost:12995
//! ```
//!
//! Every variable matters, and getting one wrong looks identical to the code being broken. The
//! probe dials over the account's **verifying** connector, so without the harness bundle the TLS
//! handshake fails and the decision fail-softs to `Password`, which is exactly what a real server
//! that refuses OAuth produces. And the harness certificates name `localhost`, so `127.0.0.1`
//! fails the same way for a different reason.

use mailcal_account::{ConnectionSecurity, ImapAuth, ImapAuthQuery, decide_imap_auth};

/// The address in `variable`, or `None` when this run is not gated on.
fn harness(variable: &str) -> Option<String> {
    std::env::var(variable).ok().filter(|addr| !addr.is_empty())
}

/// What setup decides for `email` at `imap_host`, given the issuer detection carried.
async fn decide(imap_host: String, email: &str, autoconfig_issuer: Option<&str>) -> ImapAuth {
    decide_imap_auth(&ImapAuthQuery {
        imap_host,
        imap_security: ConnectionSecurity::ImplicitTls,
        email: email.to_owned(),
        autoconfig_issuer: autoconfig_issuer.map(str::to_owned),
    })
    .await
}

#[tokio::test]
async fn the_seeded_server_offers_oauth_but_names_no_authorization_server_we_can_reach() {
    let Some(addr) = harness("MAILCAL_HARNESS_IMAP") else {
        eprintln!("skipping: MAILCAL_HARNESS_IMAP unset");
        return;
    };
    // Stalwart advertises `AUTH=PLAIN AUTH=OAUTHBEARER AUTH=XOAUTH2`, so the probe must see an
    // OAuth mechanism *and* a password. What it cannot find is a reachable authorization server:
    // Stalwart derives its issuer from its configured hostname (`https://mail.test.local`), which
    // resolves nowhere. Asserting `RegistrationNeeded` proves the whole chain ran: the dial, the
    // capability read, the OAuth branch, and the issuer search giving up honestly.
    //
    // A sign-in here would be a dead end, not an improvement: the only authorization server the
    // harness serves is the sign-in server's, whose token this server refuses. The mail server's
    // own host (`localhost`) is an issuer candidate, which is why the front serves no metadata
    // under that name.
    assert_eq!(
        decide(addr, "alice@mail.test.local", None).await,
        ImapAuth::RegistrationNeeded {
            password_also_works: true,
        },
    );
}

#[tokio::test]
async fn the_sign_in_server_offers_sign_in_through_the_issuer_its_autoconfig_names() {
    let Some(addr) = harness("MAILCAL_HARNESS_OAUTH_IMAP") else {
        eprintln!("skipping: MAILCAL_HARNESS_OAUTH_IMAP unset");
        return;
    };
    // The issuer the front's autoconfig names for `alice@localhost`, as detection hands it over.
    assert_eq!(
        decide(addr, "alice@localhost", Some("https://127.0.0.1")).await,
        ImapAuth::SignIn {
            issuer: "https://127.0.0.1".to_owned(),
            provider_label: None,
            password_also_works: true,
        },
    );
}

#[tokio::test]
async fn the_sign_in_server_without_its_autoconfig_names_no_authorization_server() {
    let Some(addr) = harness("MAILCAL_HARNESS_OAUTH_IMAP") else {
        eprintln!("skipping: MAILCAL_HARNESS_OAUTH_IMAP unset");
        return;
    };
    // Typed by hand, with no issuer carried: every candidate is `https://localhost`, where the
    // front serves the autoconfig and no metadata. The same server, one channel fewer.
    assert_eq!(
        decide(addr, "alice@localhost", None).await,
        ImapAuth::RegistrationNeeded {
            password_also_works: true,
        },
    );
}
