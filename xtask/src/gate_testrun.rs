//! Builds the test binaries once and runs them side by side.
//!
//! `cargo test` runs one test binary after another, so the workspace's run took as long as every
//! binary's added together, while most cores sat idle behind the one binary that was busy. The gate
//! builds with `--no-run`, reads what cargo built from its JSON messages, and runs the binaries
//! itself, several at once. Each binary still runs every one of its tests in one process, exactly
//! as `cargo test` runs it, from the package's directory and with the variables cargo sets.

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    process::{Child, Command},
    time::Duration,
};

use crate::{
    gate::Outcome,
    gate_exec::as_typed,
    gate_json::{self, Value},
    gate_sched::{Ctx, spawn_into},
};

/// One test binary a build produced.
#[derive(Debug, Clone)]
pub(crate) struct TestBinary {
    /// The executable.
    pub(crate) path: PathBuf,
    /// The package it tests.
    pub(crate) package: String,
    /// The package's version, which cargo hands the binary as `CARGO_PKG_VERSION`.
    version: String,
    /// The package's manifest; the binary runs from its directory.
    manifest: PathBuf,
    /// `lib`, `bin` or `test`: an integration test is told where the package's binaries are.
    kind: String,
    /// The target's name, for the log.
    name: String,
}

/// What one `cargo test --no-run` produced.
#[derive(Debug, Clone, Default)]
pub(crate) struct Build {
    /// Every test binary, in the order cargo reported them.
    pub(crate) tests: Vec<TestBinary>,
    /// Each package's executables by name, for `CARGO_BIN_EXE_<name>` and for the generators.
    bins: HashMap<String, Vec<(String, PathBuf)>>,
    /// The C ABI library `mailcal-ffi` links, when the build linked it.
    pub(crate) cdylib: Option<PathBuf>,
}

impl Build {
    /// The executable `name` of `package`, when the build produced one.
    pub(crate) fn bin(&self, package: &str, name: &str) -> Option<&Path> {
        self.bins
            .get(package)?
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, path)| path.as_path())
    }
}

/// Builds with `cargo <args> --message-format json-render-diagnostics`, in `target_dir` when one is
/// named, and keeps what it produced under `key`. `args` builds the tests without running them:
/// `test … --no-run`, or `build --profile test … --tests`.
pub(crate) fn build(
    ctx: &Ctx,
    key: &'static str,
    args: &[&str],
    target_dir: Option<&str>,
) -> Outcome {
    let messages = ctx.log_path().with_extension("json");
    let mut command = Command::new("cargo");
    command
        .args(args)
        .args(["--message-format", "json-render-diagnostics"])
        .current_dir(&ctx.root);
    if let Some(dir) = target_dir {
        command.env("CARGO_TARGET_DIR", ctx.root.join(dir));
    }
    if ctx.colour() {
        command.env("CARGO_TERM_COLOR", "always");
    }
    if !ctx.run_split(&mut command, &messages) {
        return Outcome::Fail;
    }
    let text = std::fs::read_to_string(&messages).unwrap_or_default();
    match read_build(&text) {
        Ok(build) => {
            ctx.note(&format!("{} test binaries", build.tests.len()));
            ctx.keep_build(key, build);
            Outcome::Pass
        }
        Err(why) => {
            ctx.note(&why);
            Outcome::Fail
        }
    }
}

/// What a build produced, from cargo's JSON messages.
///
/// # Errors
///
/// Fails when a message is not JSON.
pub(crate) fn read_build(text: &str) -> Result<Build, String> {
    let mut build = Build::default();
    for line in text.lines().filter(|l| l.starts_with('{')) {
        let message = gate_json::parse(line).map_err(|e| format!("cargo message: {e}"))?;
        if message.get("reason").and_then(Value::as_str) != Some("compiler-artifact") {
            continue;
        }
        let str_at = |path: &[&str]| message.at(path).and_then(Value::as_str).unwrap_or_default();
        let kinds: Vec<&str> = message
            .at(&["target", "kind"])
            .and_then(Value::as_array)
            .unwrap_or_default()
            .iter()
            .filter_map(Value::as_str)
            .collect();
        let is_test = message.at(&["profile", "test"]).and_then(Value::as_bool) == Some(true);
        let (package, version) = package_of(str_at(&["package_id"]));
        if let Some(executable) = message.get("executable").and_then(Value::as_str) {
            let entry = TestBinary {
                path: PathBuf::from(executable),
                package: package.clone(),
                version,
                manifest: PathBuf::from(str_at(&["manifest_path"])),
                kind: kinds.first().copied().unwrap_or("lib").to_owned(),
                name: str_at(&["target", "name"]).to_owned(),
            };
            if is_test {
                build.tests.push(entry);
            } else if kinds.contains(&"bin") {
                build
                    .bins
                    .entry(package)
                    .or_default()
                    .push((entry.name, entry.path));
            }
        } else if kinds.contains(&"cdylib") && package == "mailcal-ffi" {
            let files = message
                .get("filenames")
                .and_then(Value::as_array)
                .unwrap_or_default();
            build.cdylib = files
                .iter()
                .filter_map(Value::as_str)
                .find(|f| is_cdylib(f))
                .map(PathBuf::from);
        }
    }
    Ok(build)
}

/// A package's name and version from cargo's package id: `path+file:///…/name#version`, or
/// `path+file:///…/dir#name@version` when the directory is not named after the package.
fn package_of(id: &str) -> (String, String) {
    let (location, tail) = id.rsplit_once('#').unwrap_or((id, ""));
    if let Some((name, version)) = tail.split_once('@') {
        return (name.to_owned(), version.to_owned());
    }
    let name = location.rsplit('/').next().unwrap_or(location);
    (name.to_owned(), tail.to_owned())
}

fn is_cdylib(file: &str) -> bool {
    [".so", ".dylib", ".dll"]
        .iter()
        .any(|ext| file.ends_with(ext))
}

/// Runs the test binaries of the build `key` that `pick` selects, several at once, and passes when
/// every one does. `wrap` is put in front of each command line, for a binary that needs a session
/// made for it.
pub(crate) fn run(
    ctx: &Ctx,
    key: &str,
    pick: impl Fn(&TestBinary) -> bool,
    wrap: &[String],
) -> Outcome {
    let Some(build) = ctx.build(key) else {
        ctx.note(&format!("no test build called {key}"));
        return Outcome::Fail;
    };
    let mut queue: Vec<&TestBinary> = build.tests.iter().filter(|t| pick(t)).collect();
    // The longest first, so it is not the one left running alone at the end; size is a fair
    // stand-in for how many tests a binary holds.
    queue.sort_by_key(|t| std::cmp::Reverse(std::fs::metadata(&t.path).map_or(0, |m| m.len())));
    let slots = half_the_cores();
    let dir = ctx.log_path().with_extension("d");
    let _ = std::fs::create_dir_all(&dir);
    let tmp = ctx.root.join("target/tmp");
    let _ = std::fs::create_dir_all(&tmp);

    let mut running: Vec<(&TestBinary, PathBuf, Child)> = Vec::new();
    let mut failed = Vec::new();
    let mut passed = 0usize;
    let mut next = queue.into_iter();
    loop {
        while running.len() < slots {
            let Some(test) = next.next() else { break };
            let log = dir.join(format!("{}-{}.log", test.package, test.name));
            let mut command = command_for(test, &build, &tmp, wrap);
            if std::env::var_os("RUST_TEST_THREADS").is_none() {
                command.arg(format!("--test-threads={slots}"));
            }
            match spawn_into(&mut command, &log) {
                Ok(child) => running.push((test, log, child)),
                Err(e) => {
                    ctx.note(&format!("{}: could not start: {e}", test.path.display()));
                    failed.push(test.path.display().to_string());
                }
            }
        }
        if running.is_empty() {
            break;
        }
        let mut index = 0;
        while index < running.len() {
            match ctx.poll(&mut running[index].2) {
                None => index += 1,
                Some(result) => {
                    let (test, log, _) = running.swap_remove(index);
                    let output = std::fs::read_to_string(&log).unwrap_or_default();
                    let ok = matches!(result, Ok(status) if status.success());
                    let summary = output
                        .lines()
                        .rev()
                        .find(|l| l.starts_with("test result:"))
                        .unwrap_or("(no result line)");
                    ctx.note(&format!("{} {}: {summary}", test.package, describe(test)));
                    if ok {
                        passed += 1;
                    } else {
                        ctx.note(&format!("---- {} ----\n{output}", log.display()));
                        failed.push(describe(test));
                    }
                }
            }
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    ctx.note(&format!(
        "{passed} test binaries passed, {} failed",
        failed.len()
    ));
    if failed.is_empty() {
        Outcome::Pass
    } else {
        ctx.note(&format!("failed: {}", failed.join(", ")));
        Outcome::Fail
    }
}

/// Half this host's cores, and at least two: how many binaries run at once, and how many threads
/// each runs its tests on.
///
/// Not every core per binary, which is libtest's default. The app's suite hands every store call
/// to a blocking thread, and measured on an 8-core host it costs 94 CPU seconds on four threads and
/// 175 on eight, for five seconds less of wall time. Beside the gate's other work, those CPU
/// seconds are what everything else waits for. `RUST_TEST_THREADS` still overrides it.
fn half_the_cores() -> usize {
    (std::thread::available_parallelism().map_or(4, std::num::NonZero::get) / 2).max(2)
}

/// How cargo would describe the binary: `unittests` for a crate's own tests, else the target.
fn describe(test: &TestBinary) -> String {
    match test.kind.as_str() {
        "lib" => "unittests (lib)".to_owned(),
        "bin" => format!("unittests (bin {})", test.name),
        _ => format!("tests/{}", test.name),
    }
}

/// The command line `cargo test` would run the binary with, and the environment it would set.
fn command_for(test: &TestBinary, build: &Build, tmp: &Path, wrap: &[String]) -> Command {
    let mut command = match wrap.split_first() {
        Some((program, rest)) => {
            let mut c = Command::new(program);
            c.args(rest).arg(&test.path);
            c
        }
        None => Command::new(&test.path),
    };
    let dir = test.manifest.parent().unwrap_or(Path::new("."));
    as_typed(&mut command)
        .current_dir(dir)
        .env("CARGO_MANIFEST_DIR", dir)
        .env("CARGO_MANIFEST_PATH", &test.manifest)
        .env("CARGO_PKG_NAME", &test.package)
        .env("CARGO_PKG_VERSION", &test.version);
    let mut parts = test.version.splitn(3, '.');
    for name in ["MAJOR", "MINOR", "PATCH"] {
        let part = parts.next().unwrap_or("0");
        command.env(
            format!("CARGO_PKG_VERSION_{name}"),
            part.split('-').next().unwrap_or(part),
        );
    }
    if test.kind == "test" {
        command.env("CARGO_TARGET_TMPDIR", tmp);
        for (name, path) in build.bins.get(&test.package).into_iter().flatten() {
            command.env(format!("CARGO_BIN_EXE_{name}"), path);
        }
    }
    command
}

#[cfg(test)]
#[path = "gate_testrun_tests.rs"]
mod tests;
