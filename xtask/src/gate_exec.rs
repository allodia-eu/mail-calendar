//! How the gate runs one step, and how it decides whether a tool is there to run.
//!
//! Split from [`crate::gate_steps`], which holds the list itself, so neither file outgrows the line
//! ceiling this repository enforces on itself: the same rule the first check in the list applies to
//! everything else.

use std::{
    ffi::OsStr,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use crate::{
    TASKS,
    gate::{Outcome, Palette, Step},
};

/// Runs the gate's steps against one checkout.
#[derive(Debug)]
pub(crate) struct Runner<'a> {
    /// The repository root every step runs from.
    pub(crate) root: &'a Path,
    /// How to colour a heading and a verdict.
    palette: &'a Palette,
}

impl<'a> Runner<'a> {
    /// A runner for `root`.
    pub(crate) fn new(root: &'a Path, palette: &'a Palette) -> Self {
        Self { root, palette }
    }

    /// Announces a step.
    fn heading(&self, label: &str) {
        println!("{}==> {label}{}", self.palette.bold, self.palette.reset);
    }

    /// Reports a step that could not run.
    fn refused(&self, label: &str, what: &str) -> Step {
        eprintln!(
            "{}!! {label} needs {what}{}",
            self.palette.red, self.palette.reset
        );
        (label.to_owned(), Outcome::Need(what.to_owned()))
    }

    /// Runs one in-process check by name.
    pub(crate) fn named(&self, name: &str) -> Step {
        let Some(task) = TASKS.iter().find(|t| t.name == name) else {
            return (
                name.to_owned(),
                Outcome::Need(format!("no such task: {name}")),
            );
        };
        self.heading(task.label);
        let outcome = match (task.run)(self.root) {
            Ok(true) => Outcome::Pass,
            Ok(false) => Outcome::Fail,
            Err(why) => {
                eprintln!(
                    "{}!! {}: {why}{}",
                    self.palette.red, task.label, self.palette.reset
                );
                Outcome::Fail
            }
        };
        (task.label.to_owned(), outcome)
    }

    /// Runs one external command from the repository root.
    pub(crate) fn external(&self, label: &str, program: &str, args: &[&str]) -> Step {
        self.external_in(self.root, label, program, args)
    }

    /// Runs one external command from `dir`.
    pub(crate) fn external_in(
        &self,
        dir: &Path,
        label: &str,
        program: impl AsRef<OsStr>,
        args: &[&str],
    ) -> Step {
        self.heading(label);
        self.finish(label, Command::new(program).args(args).current_dir(dir))
    }

    /// Runs one external command with extra environment.
    pub(crate) fn external_env(
        &self,
        label: &str,
        program: &str,
        args: &[&str],
        env: &[(&str, &str)],
    ) -> Step {
        self.heading(label);
        let mut command = Command::new(program);
        command.args(args).current_dir(self.root);
        for (key, value) in env {
            command.env(key, value);
        }
        self.finish(label, &mut command)
    }

    /// Runs a prepared command and records the verdict.
    fn finish(&self, label: &str, command: &mut Command) -> Step {
        let ok = match command.status() {
            Ok(status) => status.success(),
            Err(e) => {
                eprintln!(
                    "{}!! {label} could not start: {e}{}",
                    self.palette.red, self.palette.reset
                );
                false
            }
        };
        if !ok {
            eprintln!(
                "{}!! {label} failed{}",
                self.palette.red, self.palette.reset
            );
        }
        (
            label.to_owned(),
            if ok { Outcome::Pass } else { Outcome::Fail },
        )
    }

    /// Runs a command whose tool may not be installed, which is a failure rather than a skip.
    ///
    /// The distinction is the whole point. A skip says "this host cannot answer that question", and
    /// a reader is entitled to treat the rest of the run as complete. A missing `reuse` or `bun`
    /// says something else: a check the pipeline does run has quietly stopped running for whoever
    /// builds. Both tools are cross-platform and cheap, and each is the only thing watching what it
    /// watches.
    pub(crate) fn optional(&self, label: &str, tool: &str, how: &str, args: &[&str]) -> Step {
        if !runnable(tool) {
            return self.refused(label, &format!("{tool}, which is not installed: {how}"));
        }
        self.external(label, tool, args)
    }

    /// Runs several commands in `dir` as one step, stopping at the first failure.
    pub(crate) fn sequence(&self, label: &str, dir: &Path, steps: &[(&str, &[&str])]) -> Step {
        self.heading(label);
        for (program, args) in steps {
            let step = self.finish(label, Command::new(program).args(*args).current_dir(dir));
            if matches!(step.1, Outcome::Fail) {
                return step;
            }
        }
        (label.to_owned(), Outcome::Pass)
    }

    /// A step this host cannot answer, said out loud.
    pub(crate) fn skip(&self, label: &str, why: &str) -> Step {
        println!(
            "{}== skipping {label}: {why}{}",
            self.palette.yellow, self.palette.reset
        );
        (label.to_owned(), Outcome::Skip(why.to_owned()))
    }

    /// The shared editor's unit suite, and the check that the committed bundle is what its sources
    /// produce.
    ///
    /// That second half is the one that matters: `editor.html` is a build output we commit, every
    /// host loads that single file, and this is the only thing standing between a source edit and a
    /// client shipping the previous bundle.
    pub(crate) fn composer(&self) -> Step {
        let label = "composer editor (typecheck + bun test + bundle freshness)";
        if !runnable("bun") {
            return self.refused(
                label,
                "bun, which is not installed: https://bun.sh, see clients/composer/package.json",
            );
        }
        self.sequence(
            label,
            &self.root.join("clients/composer"),
            &[
                ("bun", &["install", "--frozen-lockfile", "--silent"]),
                ("bun", &["run", "typecheck"]),
                ("bun", &["test"]),
                ("bun", &["run", "check"]),
            ],
        )
    }

    /// The Android suite. JDK 17 is pinned in the module, so this matches the pipeline's locale
    /// data.
    pub(crate) fn gradle(&self) -> Step {
        self.external_in(
            &self.root.join("clients/android"),
            "android (:app:test)",
            gradle_wrapper(self.root),
            &[":app:test"],
        )
    }

    /// Installs the pinned toolchain's rustfmt when it is missing.
    ///
    /// `cargo +<pin>` auto-installs a missing toolchain with its default components, without
    /// rustfmt, and then fails with "'cargo-fmt' is not installed", which reads like a bad pin
    /// rather than a missing component.
    ///
    /// Probed with `rustup toolchain list`, which is the only one of these that stays local:
    /// `rustup component list --toolchain <pin>` installs the toolchain in order to answer.
    pub(crate) fn ensure_rustfmt(&self, pin: &str) {
        let known = Command::new("rustup")
            .args(["toolchain", "list"])
            .current_dir(self.root)
            .output()
            .is_ok_and(|out| {
                String::from_utf8_lossy(&out.stdout)
                    .lines()
                    .any(|l| l.starts_with(&format!("{pin}-")))
            });
        if known
            && Command::new("rustup")
                .args(["component", "list", "--toolchain", pin, "--installed"])
                .current_dir(self.root)
                .output()
                .is_ok_and(|out| {
                    String::from_utf8_lossy(&out.stdout)
                        .lines()
                        .any(|l| l.starts_with("rustfmt"))
                })
        {
            return;
        }
        self.heading(&format!(
            "installing rustfmt for {pin} (the pin in rust-nightly.toml)"
        ));
        let _ = Command::new("rustup")
            .args([
                "toolchain",
                "install",
                pin,
                "--component",
                "rustfmt",
                "--profile",
                "minimal",
            ])
            .current_dir(self.root)
            .status();
    }
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
    let local = root.join("clients/android/local.properties");
    if std::fs::read_to_string(local)
        .is_ok_and(|t| t.lines().any(|l| l.trim_start().starts_with("sdk.dir")))
    {
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
fn runnable(tool: &str) -> bool {
    Command::new(tool)
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}
