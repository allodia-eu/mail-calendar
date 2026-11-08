//! A control drawn from the core's `ControlState` is drawn through its client's control components
//! (`docs/waiting.md`).
//!
//! Each client has one file of components that turn a `ControlState` into a held control with the
//! platform's spinner beside it. A platform control that reads the state's `enabled` itself is held
//! correctly and never shows the work running, which is the gap the components exist to close, and
//! it looks right in every review because it greys exactly when it should.
//!
//! So this refuses, outside each client's components file, a line that sets a platform control's
//! enabled state from a `ControlState`: `.disabled(` in Swift, `enabled =` in Kotlin, `IsEnabled`
//! in C# and `set_sensitive(` in Rust, beside a read of `control.enabled` or the page's
//! `remove.enabled`. A control that only opens something passes the components a state that is
//! never running (`.free(enabled)`, `copy(running = false)`, `with { Running = false }`).
//!
//! A state passed through a variable of another name is not seen, and a control drawn with no
//! state at all is not either: the check decides the shape a grep can, and the rule in
//! `docs/waiting.md` covers the rest.

use std::path::Path;

use crate::git;

/// One client: its sources, the one file allowed to draw a `ControlState` itself, and how its
/// toolkit holds a control.
struct Client {
    sources: &'static str,
    home: &'static str,
    holds: &'static [&'static str],
}

const CLIENTS: &[Client] = &[
    Client {
        sources: "clients/apple/*.swift",
        home: "clients/apple/Packages/MailcalKit/Sources/MailcalUI/ControlComponents.swift",
        holds: &[".disabled("],
    },
    Client {
        sources: "clients/android/app/src/main/*.kt",
        home: "clients/android/app/src/main/java/eu/allodia/mailcal/ControlComponents.kt",
        holds: &["enabled ="],
    },
    Client {
        sources: "clients/windows/Mailcal/*.cs",
        home: "clients/windows/Mailcal/Controls/ControlComponents.cs",
        holds: &["IsEnabled"],
    },
    Client {
        sources: "clients/linux/src/*.rs",
        home: "clients/linux/src/ui/controls.rs",
        holds: &["set_sensitive(", ".sensitive("],
    },
];

/// Runs the check. `Ok(true)` means every control drawn from a `ControlState` goes through its
/// client's components.
///
/// # Errors
///
/// Fails when git cannot list a client's sources, or when a components file is missing: a check
/// whose home has moved would pass on everything.
pub(crate) fn run(root: &Path) -> Result<bool, String> {
    let mut problems: Vec<String> = Vec::new();
    let mut read = 0;
    for client in CLIENTS {
        if !root.join(client.home).is_file() {
            return Err(format!(
                "{} is gone: is the components file still there?",
                client.home
            ));
        }
        for name in git::listed(root, &[client.sources])? {
            if name == client.home {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(root.join(&name)) else {
                continue;
            };
            read += 1;
            for (number, line) in text.lines().enumerate() {
                if holds_from_state(line, client.holds) {
                    problems.push(format!("{name}:{}: {}", number + 1, line.trim()));
                }
            }
        }
    }

    if problems.is_empty() {
        println!(
            "OK: no control in {read} client file(s) draws a ControlState past its components."
        );
        return Ok(true);
    }

    eprintln!("A control drawn from a ControlState goes through its client's components:\n");
    for problem in &problems {
        eprintln!("  {problem}");
    }
    eprintln!(
        "\nPass the state to the client's component; a control that only opens something takes \
         one that never runs. See docs/waiting.md."
    );
    Ok(false)
}

/// Whether `line` holds a platform control from a `ControlState`'s `enabled`.
fn holds_from_state(line: &str, holds: &[&str]) -> bool {
    let code = line.trim_start();
    if code.starts_with("//") || code.starts_with("///") || code.starts_with('*') {
        return false;
    }
    let lower = code.to_ascii_lowercase();
    holds.iter().any(|hold| code.contains(hold))
        && ["control.enabled", "remove.enabled"]
            .iter()
            .any(|read| lower.contains(read))
}

#[cfg(test)]
mod tests {
    use super::holds_from_state;

    const SWIFT: &[&str] = &[".disabled("];
    const KOTLIN: &[&str] = &["enabled ="];
    const CS: &[&str] = &["IsEnabled"];
    const RUST: &[&str] = &["set_sensitive(", ".sensitive("];

    #[test]
    fn a_platform_control_held_from_a_state_is_caught() {
        assert!(holds_from_state("    .disabled(!control.enabled)", SWIFT));
        assert!(holds_from_state(
            "    enabled = picker.control.enabled,",
            KOTLIN
        ));
        assert!(holds_from_state(
            "var b = new Button { IsEnabled = use.Control.Enabled };",
            CS
        ));
        assert!(holds_from_state(
            "    link.set_sensitive(switch.control.enabled);",
            RUST
        ));
        assert!(holds_from_state(
            "    enabled = page.remove.enabled,",
            KOTLIN
        ));
    }

    #[test]
    fn a_state_handed_to_a_component_is_fine() {
        assert!(!holds_from_state(
            "ControlButton(title: t, control: .free(control.enabled)) {",
            SWIFT
        ));
        assert!(!holds_from_state(
            "    control.copy(running = false),",
            KOTLIN
        ));
        // A component's own setter, not the platform's.
        assert!(!holds_from_state(
            "confirm.Enabled = use.Control.Enabled;",
            CS
        ));
        assert!(!holds_from_state(
            "let row = controls::button_row(t, switch.control.enabled, f);",
            RUST
        ));
    }

    #[test]
    fn a_control_held_from_anything_else_is_not_this_rule() {
        assert!(!holds_from_state(
            "    .disabled(model.savingSetupLinks)",
            SWIFT
        ));
        assert!(!holds_from_state("IsEnabled = target.Enabled,", CS));
        assert!(!holds_from_state(
            "// .disabled(!control.enabled) would hide the spinner",
            SWIFT
        ));
    }
}
