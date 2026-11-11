//! The Apple half of `check-test-agent`: the macOS and iOS client's agent
//! (`clients/apple/Packages/MailcalKit/Sources/MailcalUI/TestAgent/`) stays out of a shipped build.
//!
//! Swift has no attribute that compiles a call out, so the rules are the ones the Windows half
//! holds C# to:
//!
//! * every agent file, the C shim it calls SwiftUI through included, is wholly inside `#if DEBUG`
//!   (`#if DEBUG && os(macOS)` counts), so a release build compiles none of it;
//! * every line of the app that names the agent (`TestAgent.`, the shim's `mailcal_agent_`) is
//!   inside `#if DEBUG` too;
//! * `MAILCAL_TEST_AGENT`, the switch that starts it, is read in the agent alone.

use std::path::Path;

use crate::{git, report::Report, test_agent::debug_regions};

const AGENT_DIR: &str = "clients/apple/Packages/MailcalKit/Sources/MailcalUI/TestAgent/";
const SHIM_DIR: &str = "clients/apple/Packages/MailcalKit/Sources/MailcalAgentShim/";
const ENTRY: &str = "clients/apple/Packages/MailcalKit/Sources/MailcalUI/TestAgent/TestAgent.swift";
const SWITCH: &str = "MAILCAL_TEST_AGENT";
const NAMES: [&str; 2] = ["TestAgent.", "mailcal_agent_"];

/// Adds a note to `report` for each way the Apple agent could reach a shipped build, and answers
/// how many agent files it checked.
///
/// # Errors
///
/// Fails when git cannot list or read the client, or when there is no agent to check.
pub(crate) fn check(root: &Path, report: &mut Report) -> Result<usize, String> {
    let files: Vec<String> = git::listed(root, &["clients/apple"])?
        .into_iter()
        .filter(|name| matches!(crate::prose::extension(name), "swift" | "c" | "h"))
        .collect();
    if !files.iter().any(|name| name == ENTRY) {
        return Err(format!("no {ENTRY}: did the Apple test agent move?"));
    }
    let mut agent = 0;
    for name in &files {
        let text = git::read(root, name)?;
        let lines = debug_regions(&text);
        if name.starts_with(AGENT_DIR) || name.starts_with(SHIM_DIR) {
            agent += 1;
            if let Some(at) = lines
                .iter()
                .position(|(debug, code)| !debug && is_code(code))
            {
                report.note(format!(
                    "{name}:{}: code outside `#if DEBUG`; every file of the agent is debug-only",
                    at + 1
                ));
            }
            continue;
        }
        for (at, (debug, code)) in lines.iter().enumerate() {
            if *debug || is_comment(code) {
                continue;
            }
            if code.contains(SWITCH) {
                report.note(format!(
                    "{name}:{}: reads {SWITCH}, which only the agent may",
                    at + 1
                ));
            }
            if NAMES.iter().any(|agent_name| code.contains(agent_name)) {
                report.note(format!(
                    "{name}:{}: calls the test agent outside `#if DEBUG`, so a release build keeps the call",
                    at + 1
                ));
            }
        }
    }
    Ok(agent)
}

fn is_comment(line: &str) -> bool {
    line.trim_start().starts_with("//")
}

/// Whether a line outside a region would be compiled: not blank and not a comment.
fn is_code(line: &str) -> bool {
    !line.trim().is_empty() && !is_comment(line)
}
