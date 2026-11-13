//! How a gate step runs a command, and how the gate decides whether a tool is there to run.
//!
//! Split from [`crate::gate_steps`], which holds the list itself, so neither file outgrows the line
//! ceiling this repository enforces on itself: the same rule the first check in the list applies to
//! everything else.

use std::{
    collections::HashMap,
    ffi::{OsStr, OsString},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::Duration,
};

use crate::{gate::Outcome, gate_sched::Ctx};

/// What `cargo run` adds to the environment of the program it starts, besides `CARGO_PKG_*`.
///
/// The gate is that program, so every command it starts would inherit them, and a build script
/// that reads one is rerun whenever its value changes: cargo compares the value in the
/// environment cargo itself was started in. `ring` reads `CARGO_MANIFEST_DIR`, so a dependency the
/// gate built and the same one built by a `cargo test` typed by hand were two different builds,
/// and every switch between the two recompiled it and everything above it (the TLS stack, the
/// engine and this workspace) in every feature set the gate builds.
const SET_BY_CARGO_RUN: &[&str] = &[
    "CARGO",
    "CARGO_MANIFEST_DIR",
    "CARGO_MANIFEST_PATH",
    "CARGO_CRATE_NAME",
    "CARGO_BIN_NAME",
    "CARGO_PRIMARY_PACKAGE",
];

/// `command` with what `cargo run` added to the gate's environment taken back out, so it builds
/// exactly what the same command typed into a shell builds.
pub(crate) fn as_typed(command: &mut Command) -> &mut Command {
    for (name, _) in std::env::vars_os() {
        let text = name.to_string_lossy();
        if SET_BY_CARGO_RUN.contains(&text.as_ref()) || text.starts_with("CARGO_PKG_") {
            command.env_remove(&name);
        }
    }
    command
}

/// How long each step that ran took, by its label.
pub(crate) type Timings = HashMap<String, Duration>;

/// One command a step runs: a program, its arguments, where it runs and what it adds to the
/// environment.
#[derive(Debug, Clone)]
pub(crate) struct Cmd {
    program: OsString,
    args: Vec<OsString>,
    dir: Option<PathBuf>,
    env: Vec<(String, OsString)>,
}

impl Cmd {
    /// `program`, run from the repository root.
    pub(crate) fn new(program: impl AsRef<OsStr>) -> Self {
        Self {
            program: program.as_ref().to_owned(),
            args: Vec::new(),
            dir: None,
            env: Vec::new(),
        }
    }

    /// Adds arguments.
    pub(crate) fn args<S: AsRef<OsStr>>(mut self, args: impl IntoIterator<Item = S>) -> Self {
        self.args
            .extend(args.into_iter().map(|a| a.as_ref().to_owned()));
        self
    }

    /// Runs from `dir`, relative to the repository root unless absolute.
    pub(crate) fn dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.dir = Some(dir.into());
        self
    }

    /// Sets one environment variable.
    pub(crate) fn env(mut self, name: &str, value: impl AsRef<OsStr>) -> Self {
        self.env.push((name.to_owned(), value.as_ref().to_owned()));
        self
    }

    fn command(&self, ctx: &Ctx) -> Command {
        let mut command = Command::new(&self.program);
        command.args(&self.args).current_dir(
            self.dir
                .as_ref()
                .map_or_else(|| ctx.root.clone(), |d| ctx.root.join(d)),
        );
        if ctx.colour() && self.program == "cargo" {
            command.env("CARGO_TERM_COLOR", "always");
        }
        for (name, value) in &self.env {
            command.env(name, value);
        }
        command
    }
}

/// A step that runs `commands` in order, stopping at the first that fails.
pub(crate) fn commands(commands: Vec<Cmd>) -> impl FnOnce(&Ctx) -> Outcome + Send + 'static {
    move |ctx| {
        for cmd in &commands {
            if !ctx.run(&mut cmd.command(ctx)) {
                return Outcome::Fail;
            }
        }
        Outcome::Pass
    }
}

/// Installs the pinned toolchain's rustfmt when it is missing.
///
/// `cargo +<pin>` auto-installs a missing toolchain with its default components, without
/// rustfmt, and then fails with "'cargo-fmt' is not installed", which reads like a bad pin
/// rather than a missing component.
///
/// Probed with `rustup toolchain list`, which is the only one of these that stays local:
/// `rustup component list --toolchain <pin>` installs the toolchain in order to answer.
pub(crate) fn ensure_rustfmt(ctx: &Ctx, pin: &str) {
    let known = Command::new("rustup")
        .args(["toolchain", "list"])
        .current_dir(&ctx.root)
        .output()
        .is_ok_and(|out| {
            String::from_utf8_lossy(&out.stdout)
                .lines()
                .any(|l| l.starts_with(&format!("{pin}-")))
        });
    if known
        && Command::new("rustup")
            .args(["component", "list", "--toolchain", pin, "--installed"])
            .current_dir(&ctx.root)
            .output()
            .is_ok_and(|out| {
                String::from_utf8_lossy(&out.stdout)
                    .lines()
                    .any(|l| l.starts_with("rustfmt"))
            })
    {
        return;
    }
    ctx.note(&format!(
        "installing rustfmt for {pin} (the pin in rust-nightly.toml)"
    ));
    let _ = ctx.run(Command::new("rustup").current_dir(&ctx.root).args([
        "toolchain",
        "install",
        pin,
        "--component",
        "rustfmt",
        "--profile",
        "minimal",
    ]));
}

/// The Gradle wrapper this host would actually start.
///
/// Two ship, and Windows runs the other one. Probing for the POSIX name and then starting the batch
/// file is how a missing wrapper arrives as a Gradle failure several steps later instead of as the
/// skip it is: the check and the call have to name the same file.
pub(crate) fn gradle_wrapper(root: &Path) -> PathBuf {
    root.join("clients/android").join(if cfg!(windows) {
        "gradlew.bat"
    } else {
        "gradlew"
    })
}

/// Gradle's own resolution order for the Android SDK.
///
/// Without one, the suite dies at configuration with "SDK location not found": a failure that says
/// nothing about the change under test, on a host that simply does not build Android.
pub(crate) fn android_sdk_found(root: &Path) -> bool {
    if local_sdk_dir(root).is_some() {
        return true;
    }
    if ["ANDROID_HOME", "ANDROID_SDK_ROOT"]
        .iter()
        .any(|var| std::env::var_os(var).is_some_and(|v| Path::new(&v).is_dir()))
    {
        return true;
    }
    default_sdk_path().is_some_and(|p| p.is_dir())
}

/// The SDK `clients/android/local.properties` names, when that folder exists.
///
/// `.worktreeinclude` copies the file into every new worktree, so an uninstalled SDK stays named
/// in each one; trusting the line alone sends the gate into a Gradle run that cannot configure.
fn local_sdk_dir(root: &Path) -> Option<PathBuf> {
    let text = std::fs::read_to_string(root.join("clients/android/local.properties")).ok()?;
    sdk_dir_property(&text).filter(|dir| dir.is_dir())
}

/// The `sdk.dir` value as a Java properties file reads it: `#` and `!` start a comment, `=`, `:`
/// or whitespace separates the key, a backslash escapes the next character (Android Studio writes
/// `C\:\\Users` on Windows), and the last assignment wins.
fn sdk_dir_property(text: &str) -> Option<PathBuf> {
    let mut found = None;
    for line in text.lines().map(str::trim_start) {
        let Some(rest) = line.strip_prefix("sdk.dir") else {
            continue;
        };
        let value = if let Some(value) = rest.trim_start().strip_prefix(['=', ':']) {
            value
        } else if rest.starts_with(char::is_whitespace) {
            rest
        } else {
            continue;
        };
        let mut path = String::new();
        let mut chars = value.trim_start().chars();
        while let Some(c) = chars.next() {
            path.push(if c == '\\' {
                chars.next().unwrap_or(c)
            } else {
                c
            });
        }
        found = Some(PathBuf::from(path.trim_end()));
    }
    found
}

/// The per-OS directory the SDK manager installs into, which Gradle finds with nothing set.
fn default_sdk_path() -> Option<PathBuf> {
    let home = PathBuf::from(std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE"))?);
    Some(match std::env::consts::OS {
        "macos" => home.join("Library/Android/sdk"),
        "windows" => std::env::var_os("LOCALAPPDATA")
            .map_or_else(|| home.join("AppData/Local"), PathBuf::from)
            .join("Android/Sdk"),
        _ => home.join("Android/Sdk"),
    })
}

/// The pinned nightly, read from the one file the pipeline reads too.
///
/// A floating `+nightly` is not the same build: nightly rustfmt output drifts between dates, so it
/// goes green locally and red in the pipeline.
///
/// # Errors
///
/// Fails when the pin file is missing or names no channel.
pub(crate) fn nightly_channel(root: &Path) -> Result<String, String> {
    let text = std::fs::read_to_string(root.join("rust-nightly.toml"))
        .map_err(|e| format!("could not read rust-nightly.toml: {e}"))?;
    for line in text.lines() {
        let Some(rest) = line.trim().strip_prefix("channel") else {
            continue;
        };
        let Some((_, value)) = rest.split_once('=') else {
            continue;
        };
        return Ok(value.trim().trim_matches('"').to_owned());
    }
    Err("no [toolchain] channel in rust-nightly.toml".to_owned())
}

/// Whether a program can be run, by asking it rather than by looking for its name.
///
/// A probe that stops at "is there something called this on the path" passes on Windows and fails
/// on the first real call, several steps later, in whatever the tool was supposed to produce.
pub(crate) fn runnable(tool: &str) -> bool {
    Command::new(tool)
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

#[cfg(test)]
#[path = "gate_exec_tests.rs"]
mod tests;
