//! The Linux half of `check-test-agent`: the Linux client's agent (`clients/linux/src/agent.rs`)
//! stays out of a shipped build.
//!
//! One rule is enough here because Rust enforces the rest. The module is declared only under
//! `debug_assertions` or the `dev-harness` feature, which no shipped build turns on (see
//! `dev_account`), so a call to it that is not under the same gate fails a release compile, and CI
//! compiles one. What a compile cannot see is the gate itself being removed, which is this check.
//! `MAILCAL_TEST_AGENT` is read in the agent alone, so nothing else can start it.

use std::path::Path;

use crate::{git, report::Report};

const MAIN: &str = "clients/linux/src/main.rs";
const AGENT: &str = "clients/linux/src/agent";
const SWITCH: &str = "MAILCAL_TEST_AGENT";
const GATE: &str = "#[cfg(any(debug_assertions, feature = \"dev-harness\"))]";

/// Adds a note to `report` for each way the Linux agent could reach a shipped build.
///
/// # Errors
///
/// Fails when git cannot list or read the client, or when there is no agent module to check.
pub(crate) fn check(root: &Path, report: &mut Report) -> Result<usize, String> {
    let main = git::read(root, MAIN)?;
    match gated(&main) {
        None => {
            return Err(format!(
                "no `mod agent;` in {MAIN}: did the Linux test agent move?"
            ));
        }
        Some(false) => report.note(format!(
            "{MAIN}: `mod agent;` is not directly under `{GATE}`, so a shipped build compiles the \
             Linux test agent"
        )),
        Some(true) => {}
    }
    let files: Vec<String> = git::listed(root, &["clients/linux/src"])?
        .into_iter()
        .filter(|name| crate::prose::extension(name) == "rs")
        .collect();
    let mut agent = 0;
    for name in &files {
        if name.starts_with(AGENT) {
            agent += 1;
            continue;
        }
        let text = git::read(root, name)?;
        for (at, line) in text.lines().enumerate() {
            if line.contains(SWITCH) && !line.trim_start().starts_with("//") {
                report.note(format!(
                    "{name}:{}: reads {SWITCH}, which only the agent may",
                    at + 1
                ));
            }
        }
    }
    Ok(agent)
}

/// Whether `mod agent;` sits directly under the gate: `None` when there is no such declaration.
fn gated(main: &str) -> Option<bool> {
    let lines: Vec<&str> = main.lines().map(str::trim).collect();
    let at = lines.iter().position(|line| *line == "mod agent;")?;
    Some(at > 0 && lines[at - 1] == GATE)
}

#[cfg(test)]
mod tests {
    use super::gated;

    #[test]
    fn the_agent_module_is_gated_only_by_the_line_above_it() {
        let gate = "#[cfg(any(debug_assertions, feature = \"dev-harness\"))]";
        assert_eq!(gated(&format!("mod a;\n{gate}\nmod agent;\n")), Some(true));
        assert_eq!(gated("mod a;\nmod agent;\n"), Some(false));
        assert_eq!(gated(&format!("{gate}\nmod a;\nmod agent;\n")), Some(false));
        assert_eq!(gated("#[cfg(debug_assertions)]\nmod agent;\n"), Some(false));
        assert_eq!(gated("mod ui;\n"), None);
    }
}
