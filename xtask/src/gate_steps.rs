//! The gate's steps, in the order it runs them.
//!
//! How a step is actually run is in [`crate::gate_exec`]; the host-appropriate client suites are in
//! [`crate::gate_clients`]; the runner around the list is in [`crate::gate`].
//!
//! Every contract check in this list runs in this process. What is still a subprocess is work that
//! was always somebody else's program: cargo, rustup, bun, gradle, dotnet, and the checkers that
//! are still written in Python.

use std::path::Path;

use crate::{
    gate::{Outcome, Palette, Step},
    gate_clients, gate_exec,
    gate_exec::Runner,
};

/// The two checkers that stay Python, and why each does.
///
/// `check_store_copy_length.py` imports the changelog-fragment parser and the brand reader that
/// `release.py`, `flatpak_metadata.py`, `announcement.py`, `store_payload.py` and
/// `msix_manifest.py` share. Porting it would fork them, which is the one thing that file was
/// written to avoid: a fragment and a listing field must never be read by two subtly different
/// parsers. It costs a fifth of a second.
///
/// `check_user_docs.py` checks the help pages, and they are not in this tree (AGENTS.md: they
/// belong to whoever publishes the app). It skips on the first line here, so porting it would be
/// thirty kilobytes of checker for a directory this repository does not have.
const PYTHON_CHECKS: &[(&str, &str)] = &[
    (
        "store copy (field limits)",
        "scripts/ci/check_store_copy_length.py",
    ),
    ("user docs (contract)", "scripts/ci/check_user_docs.py"),
];

/// Every step, in order, run as the list is built.
pub(crate) fn all(root: &Path, clients: bool, palette: &Palette) -> Vec<Step> {
    let run = Runner::new(root, palette);
    let mut out: Vec<Step> = Vec::new();

    // 1. Formatting, on the pinned nightly: stable rustfmt cannot read this workspace's options and
    //    silently ignores them, so a floating `+nightly` goes green here and red in the pipeline.
    match gate_exec::nightly_channel(root) {
        Err(why) => out.push(("format (nightly rustfmt)".to_owned(), Outcome::Need(why))),
        Ok(pin) => {
            run.ensure_rustfmt(&pin);
            out.push(run.external(
                "format (nightly rustfmt)",
                "cargo",
                &[&format!("+{pin}"), "fmt", "--all", "--check"],
            ));
        }
    }

    // 2. The 500-line rule. Instant, and it catches the change most likely to have just been made.
    //    It sees untracked files only if you staged them, so inspect a new file yourself before
    //    calling a branch green.
    out.push(run.named("check-file-length"));

    // 3. Docs. Third, not last, as the runner's header explains. Its own unit graph means it never
    //    rides on the tests.
    out.push(run.external(
        "docs (rustdoc, warnings denied)",
        "cargo",
        &[
            "doc",
            "--workspace",
            "--exclude",
            "mailcal-linux",
            "--no-deps",
        ],
    ));

    // 4. The contract checks. All of them are searches and file reads, and together they cost less
    //    than a second.
    out.push(run.named("check-version-sync"));
    out.push(run.named("check-branding"));
    out.push(run.named("check-public-hygiene"));
    out.push(run.named("check-desktop-handoff"));
    out.push(run.named("check-portal-runtime"));
    out.push(run.named("check-license-dir"));

    // Licensing, stated once in REUSE.toml and checked per file. It fires on vendoring: a file with
    // its own SPDX header whose licence text is not in LICENSES/, and a text left there after what
    // needed it is gone. Here as well as in the pipeline because that arrives in a branch, and a
    // check only the runners get is a check nobody who builds gets.
    //
    // `pipx install reuse` alone is not enough on a host without libmagic: it installs, and then
    // every invocation dies with NoEncodingModuleError. The extra is what makes it portable.
    out.push(run.optional(
        "reuse (every file licensed)",
        "reuse",
        "pipx install 'reuse[charset-normalizer]'",
        &["lint", "--lines"],
    ));

    for (label, script) in PYTHON_CHECKS {
        out.push(run.external(label, "python3", &[script]));
    }

    out.push(run.named("check-showcase-flag"));
    out.push(run.named("check-dev-account"));

    // The two writing rules and the three silent-failure rules, all in this process. Together they
    // replaced 19.3s of Python with under a second.
    out.push(run.named("check-log-hygiene"));
    out.push(run.named("check-british-english"));
    out.push(run.named("check-dash-hygiene"));
    out.push(run.named("check-composer-labels"));
    out.push(run.named("check-surface-publish"));

    // The script suites, discovered so a new helper's tests are picked up by existing.
    out.push(run.sequence(
        "script tests (dev + ci helpers)",
        root,
        &[
            (
                "python3",
                &[
                    "-m",
                    "unittest",
                    "discover",
                    "-s",
                    "scripts/dev/tests",
                    "-q",
                ],
            ),
            (
                "python3",
                &["-m", "unittest", "discover", "-s", "scripts/ci/tests", "-q"],
            ),
        ],
    ));
    out.push(run.composer());

    // The expensive pair, last, because everything above can fail in seconds instead.
    out.push(run.external(
        "clippy (pedantic, warnings denied)",
        "cargo",
        &[
            "clippy",
            "--workspace",
            "--exclude",
            "mailcal-linux",
            "--all-targets",
            "--all-features",
            "--",
            "-D",
            "warnings",
        ],
    ));
    out.push(run.external(
        "tests (workspace)",
        "cargo",
        &["test", "--workspace", "--exclude", "mailcal-linux"],
    ));
    // `--all-features` above type-checks this build, but clippy produces nothing runnable and
    // `cargo test --workspace` runs the default feature set, so without this a test that only
    // exists behind the feature passes when its author runs it by hand and is never run again.
    out.push(run.external(
        "tests (allodia sign-in compiled in)",
        "cargo",
        &[
            "test",
            "-p",
            "mailcal-bindings",
            "--features",
            "allodia-license",
        ],
    ));

    if clients {
        out.extend(gate_clients::all(&run));
    } else {
        out.push(run.skip(
            "clients",
            "not requested: pass --clients to build Apple / Android / Windows too",
        ));
    }

    out
}

/// Prints the step list without running anything.
pub(crate) fn print_list(clients: bool) {
    println!("1  format          cargo +<rust-nightly.toml> fmt --all --check");
    println!("2  file length     cargo xtask check-file-length");
    println!("3  docs            cargo doc --workspace --exclude mailcal-linux --no-deps");
    for name in [
        "check-version-sync",
        "check-branding",
        "check-public-hygiene",
        "check-desktop-handoff",
        "check-portal-runtime",
        "check-license-dir",
    ] {
        println!("   {name:<24} in-process");
    }
    println!("   reuse                    reuse lint: required; the gate fails without it");
    for (_, script) in PYTHON_CHECKS {
        println!("   {script}");
    }
    for name in [
        "check-showcase-flag",
        "check-dev-account",
        "check-log-hygiene",
        "check-british-english",
        "check-dash-hygiene",
        "check-composer-labels",
        "check-surface-publish",
    ] {
        println!("   {name:<24} in-process");
    }
    println!("   script tests             unittest discover (scripts/dev/tests, scripts/ci/tests)");
    println!("   composer                 typecheck + bun test + bun run check: requires bun");
    println!("   clippy                   cargo clippy --workspace --all-targets --all-features");
    println!(
        "   tests                    cargo test --workspace, then -p mailcal-bindings --features \
         allodia-license"
    );
    if clients {
        println!("   clients                  Apple / Android / Windows / Linux, host permitting");
    } else {
        println!("   --clients adds           Apple / Android / Windows / Linux, host permitting");
    }
}
