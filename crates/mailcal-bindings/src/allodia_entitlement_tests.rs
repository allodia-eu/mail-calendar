use allodia_license::{
    Answer, Capability, Entitlement, GRACE_SECONDS, LAPSE_WARNING_SECONDS, Outcome,
};

use super::EntitlementState;

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

fn dir(name: &str) -> std::path::PathBuf {
    let dir =
        std::env::temp_dir().join(format!("mailcal-entitlement-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a temporary directory");
    dir
}

#[test]
fn an_answer_is_kept_beside_the_store_and_read_back_at_the_next_launch() {
    let dir = dir("kept");
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
    let dir = dir("outage");
    let state = EntitlementState::load(&dir);
    state.record(Outcome::Answered(paid()), NOW);
    state.record(Outcome::Unreachable, NOW + 1);
    assert!(EntitlementState::load(&dir).lapses_soon(NOW + GRACE_SECONDS - 1));
}

#[test]
fn a_file_that_will_not_parse_is_the_free_app() {
    let dir = dir("garbled");
    std::fs::write(dir.join(super::FILE_NAME), "not json").expect("a garbled file");
    let state = EntitlementState::load(&dir);
    assert!(!state.lapses_soon(NOW));
    assert!(state.should_refresh(NOW), "and it asks at once");
}

#[test]
fn signing_out_forgets_the_answer_here_and_on_disk() {
    let dir = dir("forget");
    let state = EntitlementState::load(&dir);
    state.record(Outcome::Answered(paid()), NOW);
    state.forget();
    assert!(!state.lapses_soon(NOW + GRACE_SECONDS - 1));
    assert!(!dir.join(super::FILE_NAME).exists());
}
