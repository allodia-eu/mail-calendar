//! The gate's steps, and what each waits for.
//!
//! How a job is scheduled is in [`crate::gate_sched`]; how a test build is read and run is in
//! [`crate::gate_testrun`]; the host-appropriate client suites are in [`crate::gate_clients`].
//!
//! Every contract check in this list runs in this process. What is still a subprocess is work that
//! was always somebody else's program: cargo, rustup, bun, gradle, dotnet, and the checkers that
//! are still written in Python.
//!
//! # Build directories
//!
//! Cargo locks a build directory for as long as it builds there, so the steps are laid out across
//! several, and they run side by side:
//!
//! - `target/check/debug`: the docs, then clippy. It is the directory kache's cargo shim gives
//!   `check` and `clippy` too, so a hand-typed `cargo clippy` and the gate's share their work.
//! - `target/debug`: the default test build, then the doctests; elsewhere than Linux, the feature
//!   build after them.
//! - `target/features/debug`: on Linux with the clients, the feature build, which selects the Linux
//!   client (below).
//! - `target/check/release`: the Linux client's release check, where there is one.
//!
//! # Feature sets
//!
//! Cargo resolves features over the packages a command selects, so `-p mailcal-bindings` and
//! `--workspace` build two variants of every crate beneath the bindings. The second test build
//! therefore selects what the first does and names its feature as `<package>/<feature>`, so it
//! rebuilds the bindings alone. The Linux client is the exception: selecting it changes the
//! features of `tokio`, `time` and a dozen more, so any build that includes it is a variant of
//! everything, and it is built once, together with the bindings' feature run.

use std::path::Path;

use crate::{
    TASKS,
    gate::Outcome,
    gate_clients,
    gate_exec::{Cmd, commands, ensure_rustfmt, nightly_channel, runnable},
    gate_sched::Plan,
    gate_testrun,
};

/// The three checkers that stay Python, and why each does.
///
/// `check_store_copy_length.py` imports the changelog-fragment parser and the brand reader that
/// `release.py`, `flatpak_metadata.py`, `announcement.py`, `store_payload.py` and
/// `msix_manifest.py` share. Porting it would fork them, which is the one thing that file was
/// written to avoid: a fragment and a listing field must never be read by two subtly different
/// parsers. It costs a fifth of a second.
///
/// `check_platform_mentions.py` reads the same notes through the same parser, for the same reason,
/// and the store pushes import its rule across two checkouts the way they already import the
/// limits. Its gate is remote and expensive: Apple has historically rejected an update whose copy
/// names a competing platform, so finding out in CI rather than here costs a round-trip.
///
/// `check_user_docs.py` checks the help pages, and they are not in this tree (AGENTS.md: they
/// belong to whoever publishes the app). It skips on the first line here, so porting it would be
/// thirty kilobytes of checker for a directory this repository does not have.
const PYTHON_CHECKS: &[(&str, &str)] = &[
    (
        "store copy (field limits)",
        "scripts/ci/check_store_copy_length.py",
    ),
    (
        "store copy (platform names)",
        "scripts/ci/check_platform_mentions.py",
    ),
    ("user docs (contract)", "scripts/ci/check_user_docs.py"),
];

/// The in-process contract checks the gate runs before the licensing lint, in order.
///
/// Named once because `all` and `print_list` both walk them: a second hand-kept copy is how a
/// check ends up registered, listed and never actually run, which is the failure the list exists
/// to make visible.
const EARLY_CHECKS: &[&str] = &[
    "check-version-sync",
    "check-branding",
    "check-public-hygiene",
    "check-desktop-handoff",
    "check-notification-registration",
    "check-portal-runtime",
    "check-store-sandbox",
    "check-line-endings",
    "check-license-dir",
];

/// The in-process contract checks the gate runs after the Python ones, in order. The last five are
/// the two writing rules and the three silent-failure rules; together they replaced 19.3s of
/// Python with under a second.
const LATE_CHECKS: &[&str] = &[
    "check-showcase-flag",
    "check-dev-account",
    "check-icons",
    "check-automation-ids",
    "check-test-agent",
    "check-control-components",
    "check-log-hygiene",
    "check-british-english",
    "check-dash-hygiene",
    "check-composer-labels",
    "check-composer-scheme",
    "check-surface-publish",
];

/// The build directory of the docs and clippy, shared with kache's cargo shim.
pub(crate) const CHECK_DIR: &str = "target/check";

/// The build directory of the test build that selects the Linux client.
const FEATURE_DIR: &str = "target/features";

/// Whether the Linux client is built here: only on Linux, and only when the clients were asked for.
pub(crate) fn linux_client(clients: bool) -> bool {
    clients && std::env::consts::OS == "linux"
}

/// Every step, in the order the summary lists them.
pub(crate) fn plan(root: &Path, clients: bool) -> Plan {
    let mut plan = Plan::default();
    let linux = linux_client(clients);
    let check_dir = root.join(CHECK_DIR);

    // Formatting, on the pinned nightly: stable rustfmt cannot read this workspace's options and
    // silently ignores them, so a floating `+nightly` goes green here and red in the pipeline.
    // `rustup run`, not `cargo +<pin>`: the `+` is read by whichever `cargo` answers, and one
    // started inside `cargo xtask` is the toolchain's own binary, which looks for rustup under
    // `CARGO_HOME` and fails when rustup lives elsewhere.
    match nightly_channel(root) {
        Err(why) => {
            plan.verdict("format (nightly rustfmt)", Outcome::Need(why));
        }
        Ok(pin) => {
            plan.add("format (nightly rustfmt)", &[], None, move |ctx| {
                ensure_rustfmt(ctx, &pin);
                let colour = if ctx.colour() { "always" } else { "never" };
                let fmt = Cmd::new("rustup")
                    .args(["run", &pin, "cargo", "fmt", "--all", "--check", "--"])
                    .args(["--color", colour]);
                commands(vec![fmt])(ctx)
            });
        }
    }

    // The contract checks, one after another in this process, cheapest first. The 500-line rule
    // leads: it catches the change most likely to have just been made. It sees untracked files only
    // if you staged them, so inspect a new file yourself before calling a branch green.
    for name in std::iter::once(&"check-file-length")
        .chain(EARLY_CHECKS)
        .chain(LATE_CHECKS)
    {
        let task = TASKS
            .iter()
            .find(|t| t.name == *name)
            .expect("a registered check");
        plan.add(
            task.label,
            &[],
            Some("in-process"),
            move |ctx| match (task.run)(&ctx.root) {
                Ok(true) => Outcome::Pass,
                Ok(false) => Outcome::Fail,
                Err(why) => {
                    eprintln!("!! {}: {why}", task.label);
                    Outcome::Fail
                }
            },
        );
    }

    // Licensing, stated once in REUSE.toml and checked per file. It fires on vendoring: a file with
    // its own SPDX header whose licence text is not in LICENSES/, and a text left there after what
    // needed it is gone. `pipx install reuse` alone is not enough on a host without libmagic: it
    // installs, and then every invocation dies with NoEncodingModuleError.
    let reuse = "reuse (every file licensed)";
    if runnable("reuse") {
        let lint = Cmd::new("reuse").args(["lint", "--lines"]);
        plan.add(reuse, &[], None, commands(vec![lint]));
    } else {
        let how = "reuse, which is not installed: pipx install 'reuse[charset-normalizer]'";
        plan.verdict(reuse, Outcome::Need(how.to_owned()));
    }
    for (label, script) in PYTHON_CHECKS {
        plan.add(
            *label,
            &[],
            None,
            commands(vec![Cmd::new("python3").args([script])]),
        );
    }
    // The script suites, discovered so a new helper's tests are picked up by existing.
    for (label, dir) in [
        ("script tests (dev helpers)", "scripts/dev/tests"),
        ("script tests (ci helpers)", "scripts/ci/tests"),
    ] {
        let suite = Cmd::new("python3").args(["-m", "unittest", "discover", "-s", dir, "-q"]);
        plan.add(label, &[], None, commands(vec![suite]));
    }
    composer(&mut plan);

    // Docs, then clippy, in their own build directory. The docs come first: `cargo test` and
    // `cargo clippy` never invoke rustdoc, and rustdoc warnings are denied here, so a link to a
    // private item survives both and is a hard error only here.
    let excluded: &[&str] = if linux {
        &[]
    } else {
        &["--exclude", "mailcal-linux"]
    };
    let docs = Cmd::new("cargo")
        .args(["doc", "--workspace", "--no-deps"])
        .args(excluded)
        .env("CARGO_TARGET_DIR", &check_dir);
    let lint_dir = Some("target/check/debug");
    plan.add(
        "docs (rustdoc, warnings denied)",
        &[],
        lint_dir,
        commands(vec![docs]),
    );
    let clippy = Cmd::new("cargo")
        .args(["clippy", "--workspace", "--all-targets", "--all-features"])
        .args(excluded)
        .args(["--", "-D", "warnings"])
        .env("CARGO_TARGET_DIR", &check_dir);
    plan.add(
        "clippy (pedantic, warnings denied)",
        &[],
        lint_dir,
        commands(vec![clippy]),
    );

    // The test builds. `cargo test --workspace` runs the default feature set, and the second build
    // is what runs a test that only exists behind a feature; without it such a test passes when its
    // author runs it by hand and is never run again.
    let test_dir = Some("target/debug");
    let built = plan.add("tests (build)", &[], test_dir, |ctx| {
        let args = ["test", "--workspace", "--exclude", "mailcal-linux"];
        match gate_testrun::build(ctx, "default", &args, None) {
            Outcome::Pass => gate_clients::stage_cdylib(ctx, "default"),
            outcome => outcome,
        }
    });
    plan.add("tests (run)", &[built], None, |ctx| {
        gate_testrun::run(ctx, "default", |_| true, &[])
    });
    let doctests = Cmd::new("cargo").args(["test", "--workspace", "--exclude", "mailcal-linux"]);
    plan.add(
        "doctests",
        &[built],
        test_dir,
        commands(vec![doctests.args(["--doc"])]),
    );
    let (label, feature_args): (&str, &[&str]) = if linux {
        (
            "tests with every feature, bindings and Linux client (build)",
            &[
                "test",
                "-p",
                "mailcal-bindings",
                "-p",
                "mailcal-linux",
                "--all-features",
            ],
        )
    } else {
        (
            "tests with the Allodia sign-in compiled in (build)",
            &[
                "test",
                "--workspace",
                "--exclude",
                "mailcal-linux",
                "--features",
                "mailcal-bindings/allodia-license",
            ],
        )
    };
    // With the Linux client selected the build is a variant of nearly every crate anyway, so it
    // gets a build directory of its own and runs beside the first instead of after it. Without it,
    // it shares the first build's directory, where all it rebuilds is the bindings.
    let (feature_dir, feature_lock) = if linux {
        (Some(FEATURE_DIR), Some("target/features/debug"))
    } else {
        (None, test_dir)
    };
    let featured = plan.add(label, &[], feature_lock, move |ctx| {
        gate_testrun::build(ctx, "features", feature_args, feature_dir)
    });
    plan.add(
        "tests with every feature, bindings (run)",
        &[featured],
        None,
        |ctx| gate_testrun::run(ctx, "features", |t| t.package == "mailcal-bindings", &[]),
    );

    if clients {
        gate_clients::all(&mut plan, root, built, featured);
    } else {
        let why = "not requested: pass --clients to build Apple / Android / Windows too";
        plan.verdict("clients", Outcome::Skip(why.to_owned()));
    }
    plan
}

/// The shared editor's unit suite, and the check that the committed bundle is what its sources
/// produce.
///
/// That second half is the one that matters: `editor.html` is a build output we commit, every
/// host loads that single file, and this is the only thing standing between a source edit and a
/// client shipping the previous bundle.
fn composer(plan: &mut Plan) {
    let label = "composer editor (typecheck + bun test + bundle freshness)";
    if !runnable("bun") {
        let how = "bun, which is not installed: https://bun.sh, see clients/composer/package.json";
        plan.verdict(label, Outcome::Need(how.to_owned()));
        return;
    }
    let bun = |args: &[&str]| Cmd::new("bun").args(args).dir("clients/composer");
    let steps = vec![
        bun(&["install", "--frozen-lockfile", "--silent"]),
        bun(&["run", "typecheck"]),
        bun(&["test"]),
        bun(&["run", "check"]),
    ];
    plan.add(label, &[], None, commands(steps));
}

/// Prints the steps without running anything.
pub(crate) fn print_list(root: &Path, clients: bool) {
    for (label, after, lock) in plan(root, clients).describe() {
        let after = if after.is_empty() {
            String::new()
        } else {
            format!("  [after: {}]", after.join("; "))
        };
        let lock = lock.map_or_else(String::new, |lock| format!("  [holds {lock}]"));
        println!("   {label}{after}{lock}");
    }
    if !clients {
        println!("   --clients adds Apple / Android / Windows / Linux, host permitting");
    }
}

#[cfg(test)]
mod tests {
    use super::{EARLY_CHECKS, LATE_CHECKS};
    use crate::TASKS;

    /// Registering a check and running it are two steps, and CI does only the first: the always-run
    /// `checks` job walks `TASKS`, while the gate walks the lists above. A check missing from them
    /// is one nobody who builds ever runs, and it goes unnoticed because CI stays green.
    #[test]
    fn every_registered_check_is_in_the_gate() {
        for task in TASKS {
            // The 500-line rule runs second, before the contract group, so it is named there.
            if task.name == "check-file-length" {
                continue;
            }
            assert!(
                EARLY_CHECKS.contains(&task.name) || LATE_CHECKS.contains(&task.name),
                "{} is registered but the gate never runs it",
                task.name
            );
        }
    }

    #[test]
    fn the_gate_names_only_registered_checks() {
        for name in EARLY_CHECKS.iter().chain(LATE_CHECKS) {
            assert!(
                TASKS.iter().any(|t| t.name == *name),
                "the gate names {name}, which is not a registered task"
            );
        }
    }
}
