//! CLI for the localisation codegen, run from each client's build script (the sibling of
//! the UniFFI `uniffi-bindgen` step):
//!
//! ```text
//! mailcal-l10n generate --target <swift|kotlin|winui|rust> --out <dir> [--root <dir>] [--package <name>]
//! ```
//!
//! `--root` defaults to the current directory (the repo root holding `messages/` +
//! `project.inlang/`). `--package` defaults per target (the Kotlin package / C# namespace).
//! A validation failure prints every problem and exits non-zero, failing the build.

use std::{path::PathBuf, process::ExitCode};

use mailcal_l10n::{Generated, Target};

fn main() -> ExitCode {
    match run() {
        Ok(files) => {
            for file in &files {
                let verb = if file.rewritten { "wrote" } else { "unchanged" };
                eprintln!("{verb} {}", file.path.display());
            }
            ExitCode::SUCCESS
        }
        Err(message) => {
            eprintln!("mailcal-l10n: {message}");
            ExitCode::FAILURE
        }
    }
}

const USAGE: &str = "usage: mailcal-l10n generate --target <swift|kotlin|winui|rust> --out <dir> [--root <dir>] [--package <name>]";

fn run() -> Result<Vec<Generated>, String> {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        Some("generate") => {}
        Some(other) => return Err(format!("unknown subcommand \"{other}\"\n{USAGE}")),
        None => return Err(USAGE.to_string()),
    }

    let mut target: Option<Target> = None;
    let mut root = PathBuf::from(".");
    let mut out: Option<PathBuf> = None;
    let mut package: Option<String> = None;

    while let Some(flag) = args.next() {
        match flag.as_str() {
            "--target" => target = Some(Target::parse(&value(&mut args, "--target")?)?),
            "--root" => root = PathBuf::from(value(&mut args, "--root")?),
            "--out" => out = Some(PathBuf::from(value(&mut args, "--out")?)),
            "--package" => package = Some(value(&mut args, "--package")?),
            other => return Err(format!("unknown flag \"{other}\"\n{USAGE}")),
        }
    }

    let target = target.ok_or("--target is required")?;
    let out = out.ok_or("--out is required")?;
    let package = package.unwrap_or_else(|| default_package(target));
    mailcal_l10n::generate(target, &root, &out, &package)
}

fn value(args: &mut impl Iterator<Item = String>, flag: &str) -> Result<String, String> {
    args.next().ok_or_else(|| format!("{flag} needs a value"))
}

fn default_package(target: Target) -> String {
    match target {
        Target::Kotlin => "eu.allodia.mailcal".to_string(),
        Target::Winui => "Allodia.Mailcal".to_string(),
        Target::Swift | Target::Rust => String::new(),
    }
}
