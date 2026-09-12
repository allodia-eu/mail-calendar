//! The repository's own task runner: the checks that gate a change, and the gate itself, as one
//! binary.
//!
//! Run the gate, every contract check at once, or one check by name, from anywhere in the
//! workspace:
//!
//! ```text
//! cargo xtask gate                  # everything, in fail-fast order
//! cargo xtask checks                # every in-process contract check, keep-going
//! cargo xtask check-file-length     # one check
//! cargo xtask --list
//! ```
//!
//! `cargo xtask` is an alias defined in `.cargo/config.toml`, so it needs nothing installed that a
//! Rust build does not already require. That is the point of the crate: these rules used to be
//! shell scripts, and a shell on Windows is an emulated x86_64 Cygwin process per command. One
//! check that starts `wc` per file cost three minutes on an arm64 developer machine and a tenth of
//! a second here.
//!
//! Each check runs in this process. The alternative shape, a loop calling `cargo xtask <check>`
//! once per check, pays cargo's freshness check every time, which costs more than the checks do.

mod branding;
mod desktop_handoff;
mod dev_account;
mod file_length;
#[cfg(test)]
mod fixture;
#[cfg(test)]
mod fixture_tests;
mod gate;
mod gate_clients;
mod gate_exec;
mod gate_steps;
mod git;
mod license_dir;
mod portal_runtime;
mod prune;
mod public_hygiene;
mod report;
mod showcase_flag;
mod showcase_lists;
mod version_sync;

use std::{
    path::{Path, PathBuf},
    process::ExitCode,
};

/// One runnable check: the name the command line uses, what it does, and how to run it.
pub(crate) struct Task {
    /// The command-line name.
    pub(crate) name: &'static str,
    /// What the gate calls this step in its summary.
    pub(crate) label: &'static str,
    /// Runs it. `Ok(true)` means the check passed.
    pub(crate) run: fn(&Path) -> Result<bool, String>,
}

impl std::fmt::Debug for Task {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Task")
            .field("name", &self.name)
            .finish_non_exhaustive()
    }
}

/// Every check, in the order the gate runs them: cheapest first, so the step most likely to fail on
/// the change just made fails first.
pub(crate) const TASKS: &[Task] = &[
    Task {
        name: "check-file-length",
        label: "file length (<= 500 lines)",
        run: file_length::run,
    },
    Task {
        name: "check-version-sync",
        label: "version sync (/VERSION)",
        run: version_sync::run,
    },
    Task {
        name: "check-branding",
        label: "branding (name + application id)",
        run: branding::run,
    },
    Task {
        name: "check-public-hygiene",
        label: "public hygiene (nothing a public repository must not carry)",
        run: public_hygiene::run,
    },
    Task {
        name: "check-desktop-handoff",
        label: "desktop handoff (portal launchers)",
        run: desktop_handoff::run,
    },
    Task {
        name: "check-portal-runtime",
        label: "portal runtime (one shared Tokio runtime)",
        run: portal_runtime::run,
    },
    Task {
        name: "check-license-dir",
        label: "licence directory (default build stands alone)",
        run: license_dir::run,
    },
    Task {
        name: "check-showcase-flag",
        label: "showcase flag contract",
        run: showcase_flag::run,
    },
    Task {
        name: "check-dev-account",
        label: "dev account contract",
        run: dev_account::run,
    },
];

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(name) = args.first() else {
        list();
        return ExitCode::FAILURE;
    };

    if matches!(name.as_str(), "--list" | "-h" | "--help") {
        list();
        return ExitCode::SUCCESS;
    }

    let root = match repo_root() {
        Ok(root) => root,
        Err(why) => {
            eprintln!("xtask: {why}");
            return ExitCode::FAILURE;
        }
    };

    match name.as_str() {
        "gate" => gate::run(&root, &args[1..]),
        "checks" => checks(&root),
        _ => single(&root, name),
    }
}

/// Runs one check by name.
fn single(root: &Path, name: &str) -> ExitCode {
    let Some(task) = TASKS.iter().find(|t| t.name == name) else {
        eprintln!("xtask: unknown task {name} (try --list)");
        return ExitCode::FAILURE;
    };
    match (task.run)(root) {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(why) => {
            eprintln!("xtask: {name}: {why}");
            ExitCode::FAILURE
        }
    }
}

/// Runs every contract check and reports all of them, rather than stopping at the first.
///
/// This is the shape the pipeline wants. A change that breaks two rules should learn both in one
/// run: the alternative sends someone round a twenty-minute loop once per rule. It is also why
/// these are one step rather than nine, now that they are one process.
fn checks(root: &Path) -> ExitCode {
    let mut failed: Vec<&str> = Vec::new();
    for task in TASKS {
        println!("==> {}", task.label);
        match (task.run)(root) {
            Ok(true) => {}
            Ok(false) => failed.push(task.name),
            Err(why) => {
                eprintln!("xtask: {}: {why}", task.name);
                failed.push(task.name);
            }
        }
    }
    if failed.is_empty() {
        println!("\nAll {} contract checks pass.", TASKS.len());
        return ExitCode::SUCCESS;
    }
    eprintln!("\n{} check(s) failed:", failed.len());
    for name in &failed {
        eprintln!("  cargo xtask {name}");
    }
    ExitCode::FAILURE
}

fn list() {
    println!("tasks:");
    println!("  {:<24} everything below, in fail-fast order", "gate");
    println!(
        "  {:<24} every check below, reporting all of them",
        "checks"
    );
    for task in TASKS {
        println!("  {:<24} {}", task.name, task.label);
    }
    println!("\ngate options: --keep-going, --list, --clients");
}

/// The repository root, so a task behaves the same from any working directory.
///
/// ⚠️ `env!("CARGO_MANIFEST_DIR")` is the wrong answer here, and wrong in a way that is invisible.
/// `.cargo/config.toml` points the build directory outside the checkout so worktrees share the
/// compile work, and the cache key does not carry the worktree: the binary one checkout compiled is
/// the binary the next one runs. A path baked in at compile time would send every check in worktree
/// B over worktree A's files, silently, and pass.
///
/// So the variable is read at run time, where cargo sets it for the run that is actually happening.
/// Outside cargo there is no variable, and the answer is found by walking up from the working
/// directory to the workspace root instead.
///
/// # Errors
///
/// Fails when neither route finds a workspace root.
fn repo_root() -> Result<PathBuf, String> {
    if let Some(manifest) = std::env::var_os("CARGO_MANIFEST_DIR")
        && let Some(parent) = Path::new(&manifest).parent()
        && is_workspace_root(parent)
    {
        return Ok(parent.to_path_buf());
    }
    let here = std::env::current_dir().map_err(|e| format!("no working directory: {e}"))?;
    for candidate in here.ancestors() {
        if is_workspace_root(candidate) {
            return Ok(candidate.to_path_buf());
        }
    }
    Err(format!(
        "no workspace root at or above {} (expected a Cargo.toml with [workspace] beside /VERSION)",
        here.display()
    ))
}

/// True when `dir` is this repository's root.
///
/// Both markers, because either alone is ordinary: a `[workspace]` manifest is every Rust
/// workspace, and `VERSION` is a file anyone may have. Together they are this tree.
fn is_workspace_root(dir: &Path) -> bool {
    dir.join("VERSION").is_file()
        && std::fs::read_to_string(dir.join("Cargo.toml"))
            .is_ok_and(|manifest| manifest.contains("[workspace]"))
}

#[cfg(test)]
mod tests {
    use super::{TASKS, is_workspace_root};

    #[test]
    fn every_task_name_is_distinct() {
        // The dispatcher takes the first match, so a duplicate would make one task unreachable
        // from the command line while still running inside `checks` and the gate.
        let mut names: Vec<&str> = TASKS.iter().map(|t| t.name).collect();
        names.sort_unstable();
        let count = names.len();
        names.dedup();
        assert_eq!(names.len(), count);
    }

    #[test]
    fn a_directory_that_is_not_the_root_is_not_mistaken_for_it() {
        let dir = std::env::temp_dir();
        assert!(!is_workspace_root(&dir));
    }
}
