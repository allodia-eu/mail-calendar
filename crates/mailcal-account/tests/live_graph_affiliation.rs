//! Gated live check that the product's own path to the engine's affiliation question
//! (`graph_affiliation`, with this crate's trust policy) tells a personal Microsoft account from a
//! work or school one. Both arms run in one invocation, because the claim is the difference; each
//! skips without its token.
//!
//! ```sh
//! T=<engine checkout>/tools/graph-oauth/.local
//! GRAPH_PERSONAL_ACCESS_TOKEN=$(python3 -c "import json;print(json.load(open('$T/tokens.json'))['access_token'])") \
//! GRAPH_WORK_ACCESS_TOKEN=$(python3 -c "import json;print(json.load(open('$T/tokens-m365.json'))['access_token'])") \
//!   cargo test -p mailcal-account --test live_graph_affiliation -- --nocapture
//! ```
//! Refresh both first (`graph-oauth refresh`, and `refresh --profile m365`): an access token lasts
//! about an hour.

use engine_api::{Affiliation, RetryConfig};

fn token(variable: &str) -> Option<String> {
    std::env::var(variable)
        .ok()
        .filter(|token| !token.is_empty())
}

#[tokio::test]
async fn a_personal_account_and_a_work_account_are_told_apart() {
    let personal = token("GRAPH_PERSONAL_ACCESS_TOKEN");
    let work = token("GRAPH_WORK_ACCESS_TOKEN");
    if personal.is_none() && work.is_none() {
        eprintln!("skipping: set GRAPH_PERSONAL_ACCESS_TOKEN and GRAPH_WORK_ACCESS_TOKEN");
        return;
    }
    if let Some(token) = personal {
        let answer = mailcal_account::graph_affiliation(token, &RetryConfig::default())
            .await
            .expect("a personal account's answer");
        // Never printed: an organisation's tenant id identifies it.
        assert!(
            answer == Affiliation::Personal,
            "a personal account read as an organisation"
        );
        eprintln!("personal account: personal");
    } else {
        eprintln!("!! NOT VERIFIED: the personal arm");
    }
    if let Some(token) = work {
        let answer = mailcal_account::graph_affiliation(token, &RetryConfig::default())
            .await
            .expect("a work account's answer");
        assert!(
            matches!(answer, Affiliation::Organization(_)),
            "a work account read as personal"
        );
        eprintln!("work account: an organisation");
    } else {
        eprintln!("!! NOT VERIFIED: the work arm");
    }
}
