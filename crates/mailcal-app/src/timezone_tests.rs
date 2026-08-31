//! State-machine tests for [`super::TimeZoneState`]: first-boot adoption, pending
//! device-zone changes, accept/dismiss/set transitions, validation, and persistence.

use std::path::PathBuf;

use engine_api::TimeZoneId;

use super::TimeZoneState;

fn ams() -> TimeZoneId {
    TimeZoneId::iana("Europe/Amsterdam").unwrap()
}

fn ny() -> TimeZoneId {
    TimeZoneId::iana("America/New_York").unwrap()
}

/// A unique temp prefs path per test (no shared global clock available).
fn temp_path(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("mailcal-tz-{tag}"));
    let _ = std::fs::remove_dir_all(&dir);
    dir.join("preferences.toml")
}

#[test]
fn first_boot_adopts_the_device_zone_without_a_pending_change() {
    let state = TimeZoneState::new(ams(), None);
    assert_eq!(state.active(), ams());
    assert_eq!(state.snapshot().active, "Europe/Amsterdam");
    assert_eq!(state.snapshot().pending_device, None);
}

#[test]
fn an_unsupported_device_zone_falls_back_to_utc() {
    let state = TimeZoneState::new(TimeZoneId::iana("Mars/Olympus").unwrap(), None);
    assert_eq!(state.active(), TimeZoneId::utc());
}

#[test]
fn reporting_a_different_zone_raises_a_pending_change_that_accept_adopts() {
    let mut state = TimeZoneState::new(ams(), None);
    // The device moved to New York: a pending change is raised, active unchanged.
    assert!(state.report_device(ny()));
    assert_eq!(state.active(), ams());
    assert_eq!(
        state.snapshot().pending_device.as_deref(),
        Some("America/New_York")
    );
    // Reporting the same pending zone again is a no-op (no re-signal).
    assert!(!state.report_device(ny()));
    // Accepting adopts it and clears the pending change.
    assert!(state.accept());
    assert_eq!(state.active(), ny());
    assert_eq!(state.snapshot().pending_device, None);
    // A second accept with nothing pending is a no-op.
    assert!(!state.accept());
}

#[test]
fn dismiss_keeps_the_current_zone_and_clears_the_prompt() {
    let mut state = TimeZoneState::new(ams(), None);
    assert!(state.report_device(ny()));
    assert!(state.dismiss());
    assert_eq!(state.active(), ams());
    assert_eq!(state.snapshot().pending_device, None);
    assert!(!state.dismiss());
}

#[test]
fn reporting_the_active_zone_clears_a_stale_pending_change() {
    let mut state = TimeZoneState::new(ams(), None);
    assert!(state.report_device(ny()));
    // Device moved back to Amsterdam before the user answered: the prompt clears.
    assert!(state.report_device(ams()));
    assert_eq!(state.snapshot().pending_device, None);
}

#[test]
fn set_changes_the_active_zone_and_clears_any_pending() {
    let mut state = TimeZoneState::new(ams(), None);
    assert!(state.report_device(ny()));
    // Picking a third zone explicitly clears the pending change and switches.
    let berlin = TimeZoneId::iana("Europe/Berlin").unwrap();
    assert!(state.set(berlin.clone()));
    assert_eq!(state.active(), berlin);
    assert_eq!(state.snapshot().pending_device, None);
    // Setting the same zone again is a no-op; an unsupported zone is ignored.
    assert!(!state.set(berlin));
    assert!(!state.set(TimeZoneId::iana("Mars/Olympus").unwrap()));
}

#[test]
fn first_boot_persists_and_a_later_launch_in_a_new_zone_prompts() {
    let path = temp_path("persist-prompt");
    // First boot in Amsterdam persists the choice (no pending).
    let first = TimeZoneState::new(ams(), Some(path.clone()));
    assert_eq!(first.active(), ams());
    assert_eq!(first.snapshot().pending_device, None);
    drop(first);
    // Relaunch on a laptop now in New York: the stored Amsterdam zone stays active,
    // and the device change is raised as a prompt.
    let second = TimeZoneState::new(ny(), Some(path.clone()));
    assert_eq!(second.active(), ams());
    assert_eq!(
        second.snapshot().pending_device.as_deref(),
        Some("America/New_York")
    );
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[test]
fn a_present_but_unreadable_prefs_file_is_not_treated_as_first_boot() {
    // A corrupt/unreadable prefs file must NOT be clobbered with the device zone (that
    // would silently lose the user's stored choice on a transient read failure). The
    // device zone is used for this run, but the existing file is left untouched.
    let path = temp_path("no-clobber");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, "this is = not = valid toml").unwrap();
    let state = TimeZoneState::new(ny(), Some(path.clone()));
    assert_eq!(state.active(), ny()); // device zone used transiently
    assert_eq!(state.snapshot().pending_device, None);
    // The corrupt file is preserved (not overwritten with the device zone).
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "this is = not = valid toml"
    );
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[test]
fn an_unsupported_device_zone_with_a_stored_zone_does_not_prompt() {
    // The OS reports a zone the bundled tzdb can't resolve while a stored zone exists.
    // The stored zone stays active and there is NO spurious "switch to Etc/UTC" prompt.
    let path = temp_path("unsupported-device");
    let first = TimeZoneState::new(ams(), Some(path.clone())); // store Amsterdam
    drop(first);
    let state = TimeZoneState::new(
        TimeZoneId::iana("Mars/Olympus").unwrap(),
        Some(path.clone()),
    );
    assert_eq!(state.active(), ams());
    assert_eq!(state.snapshot().pending_device, None);
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[test]
fn an_explicit_choice_survives_a_relaunch() {
    let path = temp_path("persist-set");
    let mut state = TimeZoneState::new(ams(), Some(path.clone()));
    state.set(ny());
    drop(state);
    // Relaunching in Amsterdam: the persisted New York choice is the active zone, and
    // because the device (Amsterdam) differs, it is offered as a pending change.
    let relaunched = TimeZoneState::new(ams(), Some(path.clone()));
    assert_eq!(relaunched.active(), ny());
    assert_eq!(
        relaunched.snapshot().pending_device.as_deref(),
        Some("Europe/Amsterdam")
    );
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}
