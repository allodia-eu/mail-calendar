use std::cell::RefCell;

use allodia_license::{
    Answer, Capability, Entitlement, Error, GRACE_SECONDS, LAPSE_WARNING_SECONDS, Outcome,
};

use super::{EntitlementState, ask_twice};

const NOW: i64 = 1_800_000_000;

fn paid() -> Answer {
    Answer {
        entitlement: Entitlement {
            plan: "personal".to_owned(),
            capabilities: [Capability::Push].into_iter().collect(),
            payment_status: Some("active".to_owned()),
            current_period_end: None,
        },
        refresh_after_seconds: 12 * 60 * 60,
    }
}

/// A directory of the test's own, and its path; the directory is removed when the guard drops.
fn dir() -> (tempfile::TempDir, std::path::PathBuf) {
    let guard = tempfile::tempdir().expect("a temporary directory");
    let path = guard.path().to_path_buf();
    (guard, path)
}

#[test]
fn an_answer_is_kept_beside_the_store_and_read_back_at_the_next_launch() {
    let (_guard, dir) = dir();
    EntitlementState::load(&dir).record(Outcome::Answered(paid()), NOW);

    let relaunched = EntitlementState::load(&dir);
    let last_week = NOW + GRACE_SECONDS - LAPSE_WARNING_SECONDS;
    assert!(!relaunched.lapses_soon(NOW));
    assert!(
        relaunched.lapses_soon(last_week),
        "the stored answer came back"
    );
}

#[test]
fn an_outage_keeps_what_was_stored() {
    let (_guard, dir) = dir();
    let state = EntitlementState::load(&dir);
    state.record(Outcome::Answered(paid()), NOW);
    state.record(Outcome::Unreachable, NOW + 1);
    assert!(EntitlementState::load(&dir).lapses_soon(NOW + GRACE_SECONDS - 1));
}

#[test]
fn a_file_that_will_not_parse_is_the_free_app() {
    let (_guard, dir) = dir();
    std::fs::write(dir.join(super::FILE_NAME), "not json").expect("a garbled file");
    let state = EntitlementState::load(&dir);
    assert!(!state.lapses_soon(NOW));
    assert!(state.should_refresh(NOW), "and it asks at once");
}

#[test]
fn signing_out_forgets_the_answer_here_and_on_disk() {
    let (_guard, dir) = dir();
    let state = EntitlementState::load(&dir);
    state.record(Outcome::Answered(paid()), NOW);
    state.forget();
    assert!(!state.lapses_soon(NOW + GRACE_SECONDS - 1));
    assert!(!dir.join(super::FILE_NAME).exists());
}

#[test]
fn a_refused_read_erases_what_was_stored() {
    let (_guard, dir) = dir();
    let state = EntitlementState::load(&dir);
    state.record(Outcome::Answered(paid()), NOW);
    state.record(Outcome::Refused, NOW + 1);
    assert!(!state.grants(&Capability::Push, NOW + 1));
    assert!(!dir.join(super::FILE_NAME).exists());
    assert!(!EntitlementState::load(&dir).grants(&Capability::Push, NOW + 1));
}

/// Asks [`ask_twice`] with the service answering each token from `answers` in turn, and a fresh
/// token minted when `mints`; what came of it, and which tokens were presented.
fn asked(answers: Vec<Result<Answer, Error>>, mints: bool) -> (Outcome, Vec<String>) {
    let answers = RefCell::new(answers.into_iter());
    let presented = RefCell::new(Vec::new());
    let outcome = ask_twice(
        "held".to_owned(),
        |token| {
            presented.borrow_mut().push(token.to_owned());
            answers.borrow_mut().next().expect("asked once too often")
        },
        || mints.then(|| "fresh".to_owned()),
    );
    (outcome, presented.into_inner())
}

/// The token held for the process may be one the service stopped taking, so a refusal is asked
/// once more with a fresh one; only a refusal of that one is the service saying no.
#[test]
fn a_refusal_is_asked_once_more_with_a_fresh_token() {
    assert_eq!(
        asked(vec![Err(Error::Unauthorized), Ok(paid())], true),
        (Outcome::Answered(paid()), vec!["held".to_owned(), "fresh".to_owned()])
    );
    assert_eq!(
        asked(vec![Err(Error::Unauthorized), Err(Error::Unauthorized)], true),
        (Outcome::Refused, vec!["held".to_owned(), "fresh".to_owned()])
    );
    assert_eq!(
        asked(vec![Err(Error::Unauthorized)], false),
        (Outcome::Unreachable, vec!["held".to_owned()]),
        "no fresh token to be had learns nothing"
    );
    assert_eq!(
        asked(
            vec![
                Err(Error::Unauthorized),
                Err(Error::Transport("down".to_owned()))
            ],
            true
        ),
        (Outcome::Unreachable, vec!["held".to_owned(), "fresh".to_owned()])
    );
    assert_eq!(
        asked(vec![Err(Error::Transport("down".to_owned()))], true),
        (Outcome::Unreachable, vec!["held".to_owned()]),
        "an outage is not asked again"
    );
}
