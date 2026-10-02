use std::{path::Path, process::Command};

use super::{Runner, as_typed};
use crate::gate::{Outcome, Palette};

const PLAIN: Palette = Palette {
    bold: "",
    red: "",
    green: "",
    yellow: "",
    reset: "",
};

#[test]
fn a_command_does_not_inherit_what_cargo_set_for_the_gate() {
    // `cargo test` sets these for this binary just as `cargo run` sets them for the gate.
    assert!(std::env::var_os("CARGO_MANIFEST_DIR").is_some());
    let mut command = Command::new("cargo");
    as_typed(&mut command);
    let removed: Vec<String> = command
        .get_envs()
        .filter(|(_, value)| value.is_none())
        .map(|(name, _)| name.to_string_lossy().into_owned())
        .collect();
    assert!(
        removed.iter().any(|name| name == "CARGO_MANIFEST_DIR"),
        "{removed:?}"
    );
    assert!(
        removed.iter().any(|name| name.starts_with("CARGO_PKG_")),
        "{removed:?}"
    );
}

#[test]
fn after_a_failure_the_rest_is_not_run_unless_asked_to_keep_going() {
    // A program that does not exist fails to start on every host.
    let missing = "mailcal-no-such-program";
    let stopping = Runner::new(Path::new("."), &PLAIN, false);
    assert!(matches!(
        stopping.external("first", missing, &[]).1,
        Outcome::Fail
    ));
    assert!(matches!(
        stopping.external("second", missing, &[]).1,
        Outcome::Skip(_)
    ));
    assert!(stopping.into_timings().contains_key("first"));

    let going = Runner::new(Path::new("."), &PLAIN, true);
    assert!(matches!(
        going.external("first", missing, &[]).1,
        Outcome::Fail
    ));
    assert!(matches!(
        going.external("second", missing, &[]).1,
        Outcome::Fail
    ));
}
