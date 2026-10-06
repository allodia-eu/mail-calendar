//! Gated live check that the product's own path to the engine's affiliation question for Google
//! (`google_affiliation`, with this crate's trust policy) tells a personal account from a Workspace
//! one, with a token holding `userinfo.email` alone, the scope every sign-in asks for. Both arms
//! run in one invocation, because the claim is the difference; each skips without its token.
//!
//! ```sh
//! # Two accounts signed in with the engine's tools/google-oauth, one personal and one Workspace;
//! # mint an access token from each narrowed to userinfo.email (`scope=` on the refresh), then:
//! GOOGLE_PERSONAL_ACCESS_TOKEN=... GOOGLE_WORKSPACE_ACCESS_TOKEN=... \
//!   cargo test -p mailcal-account --test live_google_affiliation -- --nocapture
//! ```

use engine_api::{Affiliation, RetryConfig};

fn token(variable: &str) -> Option<String> {
    std::env::var(variable)
        .ok()
        .filter(|token| !token.is_empty())
}

#[tokio::test]
async fn a_personal_account_and_a_workspace_account_are_told_apart() {
    let personal = token("GOOGLE_PERSONAL_ACCESS_TOKEN");
    let workspace = token("GOOGLE_WORKSPACE_ACCESS_TOKEN");
    if personal.is_none() && workspace.is_none() {
        eprintln!("skipping: set GOOGLE_PERSONAL_ACCESS_TOKEN and GOOGLE_WORKSPACE_ACCESS_TOKEN");
        return;
    }
    if let Some(token) = personal {
        let answer = mailcal_account::google_affiliation(token, &RetryConfig::default())
            .await
            .expect("a personal account's answer");
        // Never printed: a hosted domain identifies an organisation.
        assert!(
            answer == Affiliation::Personal,
            "a personal account read as an organisation"
        );
        eprintln!("personal account: personal");
    } else {
        eprintln!("!! NOT VERIFIED: the personal arm");
    }
    if let Some(token) = workspace {
        let answer = mailcal_account::google_affiliation(token, &RetryConfig::default())
            .await
            .expect("a Workspace account's answer");
        assert!(
            matches!(answer, Affiliation::Organization(_)),
            "a Workspace account read as personal"
        );
        eprintln!("Workspace account: an organisation");
    } else {
        eprintln!("!! NOT VERIFIED: the Workspace arm");
    }
}
