//! The client suites `--clients` adds, and only the ones this host can actually build.
//!
//! A client the host cannot build is the pipeline's to verify, and a container is not a way around
//! that: standing up a Linux image on a Mac to compile the GTK client, or reaching for the WinUI
//! client from anywhere but Windows, costs gigabytes of image and a cold compile, on a machine
//! someone is working on, to produce the answer the runner produces anyway. So each suite below
//! either runs or says out loud which question this host cannot answer.
//!
//! Each client's own build script is still that client's build system, and is invoked as one. What
//! the port removed is the shell the *gate* used to need; what remains here is Xcode's and
//! Gradle's, on the two hosts that have them.
//!
//! # The bindings library
//!
//! Gradle and the C# generator both read the bindings' C ABI library, and the gate's first test
//! build has just linked one, so they read that: [`stage_cdylib`] copies it where Gradle looks.
//! Building it again with `cargo build -p mailcal-bindings`, as each client's own script does, is a
//! third variant of every crate beneath the bindings, because cargo resolves features over the
//! packages a command selects and a test build selects their dev-dependencies too. The generated
//! bindings are the same either way: the Allodia sign-in changes the bodies of its calls, never the
//! surface.
//!
//! The generators themselves build in `target/tools`, so they never wait on the test builds' lock
//! and the clients start as soon as the library is there.

use std::path::Path;

use crate::{
    gate::Outcome,
    gate_exec::{Cmd, android_sdk_found, commands, gradle_wrapper},
    gate_sched::{Ctx, JobId, Plan},
    gate_steps::{CHECK_DIR, linux_client},
    gate_testrun,
};

/// Where the generators build, apart from the test builds.
const TOOLS_DIR: &str = "target/tools";

/// Every host-appropriate client suite. `built` stages the bindings library; `featured` holds the
/// Linux client's tests where it has any.
pub(crate) fn all(plan: &mut Plan, root: &Path, built: JobId, featured: JobId) {
    apple(plan);
    windows(plan, root, built);
    android(plan, root, built);
    linux(plan, featured);
}

/// Copies the bindings library the test build `key` linked to where `cargo build` would have put
/// it, which is where Gradle reads it.
pub(crate) fn stage_cdylib(ctx: &Ctx, key: &str) -> Outcome {
    let Some(source) = ctx.build(key).and_then(|b| b.cdylib) else {
        ctx.note("the test build linked no mailcal-bindings library to generate the bindings from");
        return Outcome::Fail;
    };
    let Some(name) = source.file_name() else {
        return Outcome::Fail;
    };
    let staged = ctx.root.join("target/debug").join(name);
    match std::fs::copy(&source, &staged) {
        Ok(_) => {
            ctx.note(&format!(
                "staged {} as {}",
                source.display(),
                staged.display()
            ));
            Outcome::Pass
        }
        Err(e) => {
            ctx.note(&format!("could not stage {}: {e}", source.display()));
            Outcome::Fail
        }
    }
}

/// Where [`stage_cdylib`] puts the library on this host.
fn staged_cdylib(root: &Path) -> String {
    let name = match std::env::consts::OS {
        "macos" => "libmailcal_bindings.dylib",
        "windows" => "mailcal_bindings.dll",
        _ => "libmailcal_bindings.so",
    };
    root.join("target/debug").join(name).display().to_string()
}

/// Apple: the *app* build, not `swift build`.
///
/// SwiftPM compiles the module as a whole, so a file missing an import still builds when a sibling
/// imports the same symbol, and only xcodebuild's batched Debug compile catches it.
///
/// The app build goes first, and that order is load-bearing rather than taste: `MailcalBindings` is
/// generated and gitignored, so on a clean checkout, or any time the cdylib's shape changed,
/// `swift test` does not fail a test, it fails to resolve the package at all. `build-and-run.sh`
/// regenerates the bindings on its way through, so running it first makes the suite test the ones
/// that match this working tree instead of whatever was there last.
fn apple(plan: &mut Plan) {
    if std::env::consts::OS != "macos" {
        plan.verdict("apple", Outcome::Skip("needs macOS + Xcode".to_owned()));
        return;
    }
    let steps: [(&str, &str, &[&str]); 4] = [
        (
            "apple (macOS app build + bindings)",
            "clients/apple/Scripts/build-and-run.sh",
            &["--macos", "--no-run"],
        ),
        (
            "apple (MailcalKit tests)",
            "clients/apple/Scripts/test-kit.sh",
            &[],
        ),
        // Its own script and its own Xcode target: the suite above runs under `swift test`, which
        // has no application for StoreKit to resolve products against and no way to load a
        // StoreKit Configuration.
        (
            "apple (StoreKit purchases)",
            "clients/apple/Scripts/test-storekit.sh",
            &[],
        ),
        (
            "apple (iOS simulator build)",
            "clients/apple/Scripts/build-and-run.sh",
            &["--iphone", "--no-run"],
        ),
    ];
    let mut previous: Option<JobId> = None;
    for (label, script, args) in steps {
        let after: Vec<JobId> = previous.into_iter().collect();
        let step = commands(vec![Cmd::new(script).args(args)]);
        previous = Some(plan.add(label, &after, Some("xcode"), step));
    }
}

/// Android: JDK 17 is pinned in the module, so this matches the pipeline's locale data.
///
/// Gated on the SDK as well as on the wrapper, and in the order Gradle resolves it. Without one,
/// `:app:test` dies at configuration with "SDK location not found": a failure that says nothing
/// about the change under test, on a host that simply does not build Android. A gate that is red
/// for a reason nobody can act on is one people stop reading.
///
/// **Both APKs.** The module carries a `play` flavour and a `foss` one, and `:app:test` builds and
/// runs each, so the build that carries no Google library is exercised here rather than first at
/// an F-Droid submission.
fn android(plan: &mut Plan, root: &Path, built: JobId) {
    if !gradle_wrapper(root).is_file() {
        plan.verdict(
            "android",
            Outcome::Skip("no Gradle wrapper in clients/android".to_owned()),
        );
        return;
    }
    if !android_sdk_found(root) {
        let why =
            "no Android SDK: set ANDROID_HOME, or sdk.dir in clients/android/local.properties";
        plan.verdict("android", Outcome::Skip(why.to_owned()));
        return;
    }
    plan.add("android (:app:test)", &[built], None, |ctx| {
        let gradle = Cmd::new(gradle_wrapper(&ctx.root))
            .args([":app:test", "-x", "cargoBuildBindings"])
            .dir("clients/android")
            .env("CARGO_TARGET_DIR", ctx.root.join(TOOLS_DIR));
        commands(vec![gradle])(ctx)
    });
}

/// Windows: the pure `net10.0` half runs anywhere dotnet does, including macOS. The WinUI app and
/// the UI Automation suite need a Windows desktop session and are not covered here.
///
/// The regeneration is not optional. `Mailcal.Tests` links the generated C# bindings and `L10n.cs`,
/// and on Windows `build-and-run.ps1` regenerates both before it ever calls `dotnet test`. Off
/// Windows nothing does, so without it the suite compiles against whatever shape the bindings had
/// the last time somebody remembered, which after an FFI or catalog change is a green run proving
/// nothing.
fn windows(plan: &mut Plan, root: &Path, built: JobId) {
    if !dotnet_answers(root, "--list-sdks") {
        plan.verdict("windows", Outcome::Skip("no dotnet on PATH".to_owned()));
        return;
    }
    // `--version` resolves the SDK from the working directory up, so it fails inside the checkout
    // when only another major is installed, and the steps below would fail with it for a reason
    // that says nothing about the change.
    if !dotnet_answers(root, "--version") {
        let why = "the .NET SDK global.json pins: `dotnet --version` in the checkout names it, and a \
                   newer major installed beside it does not stand in";
        plan.verdict("windows", Outcome::Need(why.to_owned()));
        return;
    }
    let regenerated = plan.add(
        "windows (regenerate C# bindings + l10n)",
        &[built],
        None,
        |ctx| {
            let tools = ctx.root.join(TOOLS_DIR);
            let library = staged_cdylib(&ctx.root);
            commands(vec![
                Cmd::new("cargo")
                    .args([
                        "run",
                        "-q",
                        "-p",
                        "mailcal-bindgen-cs",
                        "--",
                        "--library",
                        &library,
                    ])
                    .args(["--out-dir", "clients/windows/Generated"])
                    .env("CARGO_TARGET_DIR", &tools),
                Cmd::new("cargo")
                    .args([
                        "run",
                        "-q",
                        "-p",
                        "mailcal-l10n",
                        "--",
                        "generate",
                        "--target",
                        "winui",
                    ])
                    .args(["--root", ".", "--out", "clients/windows/Mailcal"])
                    .env("CARGO_TARGET_DIR", &tools),
            ])(ctx)
        },
    );
    let dotnet: [(&str, &[&str]); 3] = [
        // `--project`, not a bare directory: under the Microsoft.Testing.Platform runner
        // `global.json` selects, `dotnet test` refuses a positional path.
        (
            "windows (Mailcal.Tests)",
            &["test", "--project", "clients/windows/Mailcal.Tests"],
        ),
        // MailcalVerify is the only thing off Windows that compiles `Verify.cs` against the
        // freshly generated bindings, so an FFI-shape change fails here instead of on the
        // Windows box.
        (
            "windows (MailcalVerify compiles)",
            &["build", "clients/windows/MailcalVerify", "--nologo"],
        ),
        // The agent UI suites run against a live app, but they reference none of it, so they
        // compile anywhere the SDK does.
        (
            "windows (Mailcal.UiTests compiles)",
            &["build", "clients/windows/Mailcal.UiTests", "--nologo"],
        ),
    ];
    for (label, args) in dotnet {
        let step = commands(vec![Cmd::new("dotnet").args(args)]);
        plan.add(label, &[regenerated], Some("dotnet"), step);
    }
    plan.verdict(
        "windows (WinUI app + UI suites)",
        Outcome::Skip(
            "needs a Windows desktop session: run build-and-run.ps1 and uitests/run-ui-tests.ps1 \
             there"
                .to_owned(),
        ),
    );
}

fn dotnet_answers(root: &Path, arg: &str) -> bool {
    std::process::Command::new("dotnet")
        .arg(arg)
        .current_dir(root)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

/// Linux's GTK client, which is excluded from the workspace gate on every other host.
///
/// Its clippy and its docs run with the workspace's, in the same commands: on this host the gate's
/// docs and clippy select it. Its tests are built with the bindings' feature run.
///
/// The tests run on a private headless compositor, never the developer's desktop. Left on the
/// desktop they drive the live one: windows flash on screen, and a test that pumps the main loop
/// dispatches Wayland events for surfaces an earlier test already destroyed, which segfaults
/// inside libwayland-client. The pipeline has no session at all and needs one made for it either
/// way. `scripts/dev/with-headless-session.sh` owns that.
///
/// Clippy sees `--all-features` in debug, which compiles the dev fixtures and the showcase hooks.
/// The release check compiles what the Flatpak and the snap compile, with default features, where
/// something only those hooks use is dead code and, under `-D warnings`, a failed release. It is a
/// check rather than clippy: rustc's lints are what fail that build, and clippy's pedantic set
/// reads code `cfg` has trimmed as code that could be simpler.
fn linux(plan: &mut Plan, featured: JobId) {
    if !linux_client(true) {
        let why = "needs a Linux host with GTK 4.22+/libadwaita 1.9+";
        plan.verdict("linux (mailcal-linux)", Outcome::Skip(why.to_owned()));
        return;
    }
    plan.add(
        "linux (release check)",
        &[],
        Some("target/check/release"),
        |ctx| {
            let check = Cmd::new("cargo")
                .args(["check", "-p", "mailcal-linux", "--release"])
                .env("CARGO_TARGET_DIR", ctx.root.join(CHECK_DIR));
            commands(vec![check])(ctx)
        },
    );
    plan.add("linux (tests)", &[featured], None, |ctx| {
        let session = ctx.root.join("scripts/dev/with-headless-session.sh");
        let wrap = ["bash".to_owned(), session.display().to_string()];
        gate_testrun::run(ctx, "features", |t| t.package == "mailcal-linux", &wrap)
    });
}
