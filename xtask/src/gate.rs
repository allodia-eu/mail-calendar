//! The local gate, as one command, in fail-fast order.
//!
//! ```text
//! cargo xtask gate                 # the workspace gate
//! cargo xtask gate --clients       # ...plus every client this host can actually build
//! cargo xtask gate --keep-going    # run everything and summarise, instead of stopping
//! cargo xtask gate --list          # print the steps and exit
//! ```
//!
//! This is the executable form of AGENTS.md → "Building & verifying". That prose is still the
//! documentation; this is the thing you run, so the gate cannot be half-run from memory. Run it
//! before the first push of a branch. The pipeline costs real money and macOS runners bill at 10x,
//! so a pull request is where you *confirm* a green build, not where you discover one.
//!
//! # Why the order is what it is
//!
//! Cheapest first, and the doc build early. The order is not the pipeline's; it is chosen so the
//! step most likely to fail on the change just made fails *first*. That matters most for the doc
//! gate: `cargo test` and `cargo clippy` never invoke rustdoc, so a broken doc link survives both,
//! and this workspace denies rustdoc warnings, so a link to a private item is a hard error while
//! the bracketed form is the idiomatic thing to type. Run last, that error lands minutes after the
//! sentence that caused it. A warm `cargo doc --no-deps` over this workspace is under six seconds.
//!
//! # What it deliberately does not do
//!
//! Nothing here needs a device, a simulator, an emulator or Docker. The client UI suites that do
//! stay out, because a gate that cannot run is a gate people stop running. `--clients` adds only
//! the headless, host-appropriate ones, and says out loud which it skipped and why: a skip that
//! looks like a pass is the failure this whole file exists to prevent.
//!
//! The steps themselves are in [`crate::gate_steps`].

use std::{io::IsTerminal, path::Path, process::ExitCode};

use crate::{gate_steps, prune};

/// What became of one step.
#[derive(Debug)]
pub(crate) enum Outcome {
    /// It ran and passed.
    Pass,
    /// It ran and failed.
    Fail,
    /// A question this host cannot answer, so the rest of the run is still complete.
    Skip(String),
    /// A tool that is installable right here, whose absence would take a check with it. That is a
    /// failure, not a skip: it means a check the pipeline does run has quietly stopped running for
    /// whoever builds, and they will find out from a red pipeline instead.
    Need(String),
}

/// One finished step: what it was called, and how it went.
pub(crate) type Step = (String, Outcome);

/// The escape sequences, or nothing when the output is not a terminal.
#[derive(Debug)]
pub(crate) struct Palette {
    /// Emphasis, for a step heading.
    pub(crate) bold: &'static str,
    /// A failure.
    pub(crate) red: &'static str,
    /// A pass.
    pub(crate) green: &'static str,
    /// A skip.
    pub(crate) yellow: &'static str,
    /// Back to the terminal's own colours.
    pub(crate) reset: &'static str,
}

impl Palette {
    /// Colours when stdout is a terminal, and nothing when it is a file or a pipeline log.
    pub(crate) fn detect() -> Self {
        if std::io::stdout().is_terminal() {
            Self {
                bold: "\x1b[1m",
                red: "\x1b[31m",
                green: "\x1b[32m",
                yellow: "\x1b[33m",
                reset: "\x1b[0m",
            }
        } else {
            Self {
                bold: "",
                red: "",
                green: "",
                yellow: "",
                reset: "",
            }
        }
    }
}

/// Runs the gate.
pub(crate) fn run(root: &Path, args: &[String]) -> ExitCode {
    let keep_going = args.iter().any(|a| a == "--keep-going");
    let clients = args.iter().any(|a| a == "--clients");
    if args.iter().any(|a| a == "--list") {
        gate_steps::print_list(clients);
        return ExitCode::SUCCESS;
    }
    for arg in args {
        if !matches!(arg.as_str(), "--keep-going" | "--clients" | "--list") {
            eprintln!("gate: unknown option {arg} (try --list)");
            return ExitCode::FAILURE;
        }
    }

    let palette = Palette::detect();
    let mut results: Vec<Step> = Vec::new();
    let mut failed = false;

    for step in gate_steps::all(root, clients, &palette) {
        let stop = matches!(step.1, Outcome::Fail | Outcome::Need(_));
        failed |= stop;
        results.push(step);
        if stop && !keep_going {
            break;
        }
    }

    summary(&results, &palette);
    if failed {
        // The cache is left warm on purpose: a red gate means you are still iterating, and that is
        // exactly when it is worth its disk.
        println!(
            "\n{}The gate is RED.{} Fix the above before pushing. The pipeline is not the place to \
             find this.",
            palette.red, palette.reset
        );
        return ExitCode::FAILURE;
    }
    prune::incremental(root);
    println!("\n{}The gate is GREEN.{}", palette.green, palette.reset);
    ExitCode::SUCCESS
}

fn summary(results: &[Step], palette: &Palette) {
    println!("\n{}---- gate summary ----{}", palette.bold, palette.reset);
    for (label, outcome) in results {
        let (tag, colour) = match outcome {
            Outcome::Pass => ("PASS", palette.green),
            Outcome::Fail => ("FAIL", palette.red),
            Outcome::Skip(_) => ("SKIP", palette.yellow),
            Outcome::Need(_) => ("NEED", palette.red),
        };
        match outcome {
            Outcome::Skip(why) | Outcome::Need(why) => {
                println!("{colour}{tag}{}  {label}: {why}", palette.reset);
            }
            _ => println!("{colour}{tag}{}  {label}", palette.reset),
        }
    }
}
