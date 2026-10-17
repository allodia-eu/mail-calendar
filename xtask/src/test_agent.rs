//! Fails when the Windows app's test agent could reach a release build.
//!
//! The agent (`clients/windows/Mailcal/Services/TestAgent*.cs`) answers anyone on the machine who
//! names its pipe, and acts on the app's windows for them. It is for the UI suites alone, so a
//! shipped build must not contain it, and nothing at runtime can be trusted to turn it off: a
//! debug-only switch that leaks into release looks like nothing at all. Three rules hold it out:
//!
//! * every agent file but the entry points is wholly inside `#if DEBUG`;
//! * every method of the agent declared outside `#if DEBUG` carries `[Conditional("DEBUG")]`, so
//!   the app's calls to it are compiled out of a release build along with its body;
//! * `MAILCAL_TEST_AGENT`, the switch that starts it, is read nowhere outside `#if DEBUG`.

use std::path::Path;

use crate::{git, report::Report};

/// Where the agent and the app that calls it live.
const APP: &str = "clients/windows/Mailcal";

/// The file holding the entry points the rest of the app calls.
const ENTRY: &str = "clients/windows/Mailcal/Services/TestAgent.cs";

/// The switch that starts the agent.
const SWITCH: &str = "MAILCAL_TEST_AGENT";

/// Runs the check. `Ok(true)` means the agent cannot reach a release build.
///
/// # Errors
///
/// Fails when git cannot list the app, or when there is no agent to check: a rule over no files is
/// one that cannot fail.
pub(crate) fn run(root: &Path) -> Result<bool, String> {
    let files: Vec<String> = git::listed(root, &[APP])?
        .into_iter()
        .filter(|name| crate::prose::extension(name) == "cs")
        .collect();
    let agent: Vec<&String> = files.iter().filter(|name| is_agent(name)).collect();
    if !agent.iter().any(|name| name.as_str() == ENTRY) {
        return Err(format!("no {ENTRY}: did the test agent move?"));
    }

    let mut report = Report::new();
    for name in &files {
        let text = git::read(root, name)?;
        let lines = debug_regions(&text);
        if is_agent(name)
            && name.as_str() != ENTRY
            && lines.iter().any(|(debug, code)| !debug && is_code(code))
        {
            report.note(format!(
                "{name}: has code outside `#if DEBUG`; every agent file but {ENTRY} is debug-only"
            ));
        }
        if name.as_str() == ENTRY {
            for (at, (debug, code)) in lines.iter().enumerate() {
                if !debug && declares_method(code) && !conditional_above(&lines, at) {
                    report.note(format!(
                        "{name}:{}: `{}` is outside `#if DEBUG` without [Conditional(\"DEBUG\")], so a release build keeps its calls",
                        at + 1,
                        code.trim()
                    ));
                }
            }
        }
        for (at, (debug, code)) in lines.iter().enumerate() {
            if !debug && code.contains(SWITCH) && !code.trim_start().starts_with("//") {
                report.note(format!(
                    "{name}:{}: reads {SWITCH} outside `#if DEBUG`",
                    at + 1
                ));
            }
        }
    }

    if !report.failed() {
        println!(
            "Test agent: {} file(s), none of it in a release build.",
            agent.len()
        );
        return Ok(true);
    }
    eprintln!("The Windows test agent could reach a release build:\n");
    report.emit();
    eprintln!(
        "\nThe agent acts on the app for anyone who names its pipe; it is for the UI suites alone."
    );
    Ok(false)
}

fn is_agent(name: &str) -> bool {
    name.rsplit('/')
        .next()
        .is_some_and(|file| file.starts_with("TestAgent"))
}

/// Each line, with whether it is inside an `#if DEBUG` region. A directive line counts as outside,
/// so it is never mistaken for code either way.
pub(crate) fn debug_regions(text: &str) -> Vec<(bool, &str)> {
    let mut open: Vec<bool> = Vec::new();
    let mut lines = Vec::new();
    for line in text.lines() {
        let directive = line.trim_start();
        if let Some(condition) = directive.strip_prefix("#if ") {
            open.push(condition.trim() == "DEBUG");
            lines.push((false, ""));
            continue;
        }
        if directive.starts_with("#else") {
            if let Some(top) = open.last_mut() {
                *top = !*top;
            }
            lines.push((false, ""));
            continue;
        }
        if directive.starts_with("#endif") {
            open.pop();
            lines.push((false, ""));
            continue;
        }
        lines.push((open.iter().any(|debug| *debug), line));
    }
    lines
}

/// Whether a line outside a region is code that would be compiled: not blank, not a comment, not
/// a `using`, not the namespace line.
fn is_code(line: &str) -> bool {
    let trimmed = line.trim_start();
    !(trimmed.is_empty()
        || trimmed.starts_with("//")
        || trimmed.starts_with("using ")
        || trimmed.starts_with("namespace "))
}

/// Whether a line declares a method the rest of the app could call.
fn declares_method(line: &str) -> bool {
    let trimmed = line.trim_start();
    (trimmed.starts_with("internal static ") || trimmed.starts_with("public static "))
        && trimmed.contains('(')
        && !trimmed.contains(" class ")
        && !trimmed.contains('=')
}

/// Whether the attribute lines directly above a declaration include `[Conditional("DEBUG")]`.
fn conditional_above(lines: &[(bool, &str)], at: usize) -> bool {
    lines[..at]
        .iter()
        .rev()
        .map(|(_, code)| code.trim_start())
        .take_while(|code| code.starts_with('[') || code.starts_with("///"))
        .any(|code| code.starts_with("[Conditional(\"DEBUG\")]"))
}

#[cfg(test)]
#[path = "test_agent_tests.rs"]
mod tests;
