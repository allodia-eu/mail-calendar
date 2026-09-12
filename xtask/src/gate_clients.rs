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

use crate::{
    gate::Step,
    gate_exec::{Runner, android_sdk_found, gradle_wrapper},
};

/// Every host-appropriate client suite.
pub(crate) fn all(run: &Runner<'_>) -> Vec<Step> {
    let mut out = Vec::new();
    out.extend(apple(run));
    out.push(android(run));
    out.extend(windows(run));
    out.extend(linux(run));
    out
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
fn apple(run: &Runner<'_>) -> Vec<Step> {
    if std::env::consts::OS != "macos" {
        return vec![run.skip("apple", "needs macOS + Xcode")];
    }
    vec![
        run.external(
            "apple (macOS app build + bindings)",
            "clients/apple/Scripts/build-and-run.sh",
            &["--macos", "--no-run"],
        ),
        run.external(
            "apple (MailcalKit tests)",
            "clients/apple/Scripts/test-kit.sh",
            &[],
        ),
        run.external(
            "apple (iOS simulator build)",
            "clients/apple/Scripts/build-and-run.sh",
            &["--iphone", "--no-run"],
        ),
    ]
}

/// Android: JDK 17 is pinned in the module, so this matches the pipeline's locale data.
///
/// Gated on the SDK as well as on the wrapper, and in the order Gradle resolves it. Without one,
/// `:app:test` dies at configuration with "SDK location not found": a failure that says nothing
/// about the change under test, on a host that simply does not build Android. A gate that is red
/// for a reason nobody can act on is one people stop reading.
fn android(run: &Runner<'_>) -> Step {
    let wrapper = gradle_wrapper(run.root);
    if !wrapper.is_file() {
        return run.skip("android", "no Gradle wrapper in clients/android");
    }
    if !android_sdk_found(run.root) {
        return run.skip(
            "android",
            "no Android SDK: set ANDROID_HOME, or sdk.dir in clients/android/local.properties",
        );
    }
    run.gradle()
}

/// Windows: the pure `net10.0` half runs anywhere dotnet does, including macOS. The WinUI app and
/// the UI Automation suite need a Windows desktop session and are not covered here.
///
/// The two regeneration steps are not optional. `Mailcal.Tests` links the generated C# bindings and
/// `L10n.cs`, and on Windows `build-and-run.ps1` regenerates both before it ever calls `dotnet
/// test`. Off Windows nothing does, so without these the suite compiles against whatever shape the
/// bindings had the last time somebody remembered, which after an FFI or catalog change is a green
/// run proving nothing.
fn windows(run: &Runner<'_>) -> Vec<Step> {
    if !dotnet_present() {
        return vec![run.skip("windows", "no dotnet on PATH")];
    }
    let cdylib = cdylib_path();
    vec![
        run.sequence(
            "windows (regenerate C# bindings + l10n)",
            run.root,
            &[
                ("cargo", &["build", "-p", "mailcal-bindings"]),
                (
                    "cargo",
                    &[
                        "run",
                        "-q",
                        "-p",
                        "mailcal-bindgen-cs",
                        "--",
                        "--library",
                        &cdylib,
                        "--out-dir",
                        "clients/windows/Generated",
                    ],
                ),
                (
                    "cargo",
                    &[
                        "run",
                        "-q",
                        "-p",
                        "mailcal-l10n",
                        "--",
                        "generate",
                        "--target",
                        "winui",
                        "--root",
                        ".",
                        "--out",
                        "clients/windows/Mailcal",
                    ],
                ),
            ],
        ),
        run.external(
            "windows (Mailcal.Tests)",
            "dotnet",
            &["test", "clients/windows/Mailcal.Tests", "--nologo"],
        ),
        // MailcalVerify is the only thing off Windows that compiles `Verify.cs` against the
        // freshly generated bindings. It is a plain net10.0 console app, so the build
        // costs seconds and catches an FFI-shape change here instead of on the Windows
        // box.
        run.external(
            "windows (MailcalVerify compiles)",
            "dotnet",
            &["build", "clients/windows/MailcalVerify", "--nologo"],
        ),
        run.skip(
            "windows (WinUI app + uitests)",
            "needs a Windows desktop session: run build-and-run.ps1 and uitests/run-ui-tests.ps1 \
             there",
        ),
    ]
}

/// Whether the .NET SDK answers, rather than whether something called `dotnet` is on the path.
fn dotnet_present() -> bool {
    std::process::Command::new("dotnet")
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

/// Where the bindings cdylib lands on this host.
fn cdylib_path() -> String {
    match std::env::consts::OS {
        "macos" => "target/debug/libmailcal_bindings.dylib".to_owned(),
        "windows" => "target/debug/mailcal_bindings.dll".to_owned(),
        _ => "target/debug/libmailcal_bindings.so".to_owned(),
    }
}

/// Linux's GTK client, which is excluded from the workspace gate on every other host.
///
/// `GDK_BACKEND=x11` is what makes `xvfb-run` mean anything. GDK prefers Wayland whenever
/// `WAYLAND_DISPLAY` is set, and xvfb-run does not clear it, so on a Wayland desktop the suite
/// ignores the X server it just started and drives the developer's live compositor instead: windows
/// flash on screen, and a test that pumps the main loop dispatches Wayland events for surfaces an
/// earlier test already destroyed, which segfaults inside libwayland-client. The pipeline has no
/// session at all, so this only ever bites the person running the gate by hand.
fn linux(run: &Runner<'_>) -> Vec<Step> {
    if std::env::consts::OS != "linux" {
        return vec![run.skip(
            "linux (mailcal-linux)",
            "needs a Linux host with GTK 4.14+/libadwaita 1.5+",
        )];
    }
    vec![
        run.external(
            "linux (clippy)",
            "cargo",
            &[
                "clippy",
                "-p",
                "mailcal-linux",
                "--all-targets",
                "--all-features",
                "--",
                "-D",
                "warnings",
            ],
        ),
        run.external_env(
            "linux (tests)",
            "xvfb-run",
            &[
                "--auto-servernum",
                "cargo",
                "test",
                "-p",
                "mailcal-linux",
                "--all-features",
            ],
            &[("GDK_BACKEND", "x11")],
        ),
        run.external(
            "linux (docs)",
            "cargo",
            &["doc", "-p", "mailcal-linux", "--no-deps"],
        ),
    ]
}
