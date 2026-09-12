//! Guards the showcase (store-screenshot) flag contract.
//!
//! Showcase mode's whole safety property is "the app on screen is showing fictional mail, never the
//! developer's". Nothing about a screenshot reveals whether that held, so the ways it silently
//! breaks are worth machine-checking. Each assertion below is a bug that shipped or was one edit
//! away from shipping.

use std::path::Path;

use crate::{
    git::{self, Kind},
    report::Report,
    showcase_lists as lists,
};

/// The Linux client's showcase module.
const LINUX_SHOWCASE: &str = "clients/linux/src/showcase.rs";

/// The capture driver every other path is compared against.
const SHOWCASE_SH: &str = "scripts/dev/showcase.sh";

/// The Windows capture driver, the one no non-Windows developer can run.
const SHOWCASE_PS1: &str = "clients/windows/showcase.ps1";

/// The line the core logs from inside the showcase boot, matched by every capture path.
const MARKER: &str = "showcase (screenshot) app starting (in-memory engine, seeded";

/// Runs the check. `Ok(true)` means the contract holds.
///
/// # Errors
///
/// Propagates a git or filesystem failure.
pub(crate) fn run(root: &Path) -> Result<bool, String> {
    let mut report = Report::new();

    retired_name_is_gone(root, &mut report)?;
    launcher_agrees(root, &mut report)?;
    release_gated(root, &mut report)?;
    marker_in_step(root, &mut report)?;

    let showcase_sh = git::read(root, SHOWCASE_SH)?;
    let showcase_ps1 = git::read(root, SHOWCASE_PS1)?;
    let locales = same_locales(root, &showcase_sh, &showcase_ps1, &mut report)?;
    let screens = same_screens(&showcase_sh, &showcase_ps1, &mut report);
    let (shot, accepted) = same_appearances(&showcase_sh, &showcase_ps1, &mut report);
    let (shared, macos) = doc_screens_reachable(root, &showcase_sh, &mut report)?;
    linux_screens_reachable(root, &showcase_sh, &mut report)?;
    no_device_theme(root, &mut report)?;

    if report.failed() {
        report.emit();
        return Ok(false);
    }

    println!(
        "OK: the showcase flag contract holds (no retired name, launcher agrees, Release-gated,\n    \
         marker in step, all three capture paths offer: {}\n    and both offer the screens: {}).\n    \
         and the appearances: {}(driver accepts: {}).\n    \
         Documentation screens reach their drivers: {}| macOS also {}",
        lists::render(&locales),
        lists::render(&screens),
        lists::render(&shot),
        lists::render(&accepted),
        lists::render(&shared),
        lists::render(&macos),
    );
    Ok(true)
}

/// The flag was once named otherwise, and one launcher kept reading the old name, so the banner
/// warning "this launch shows the showcase dataset, NOT your accounts" tested a variable nothing
/// sets and could never fire. The launcher stayed silent exactly when it had the most to say.
fn retired_name_is_gone(root: &Path, report: &mut Report) -> Result<(), String> {
    let hits = git::grep(
        root,
        Kind::Fixed,
        "ALLODIA_DEMO",
        &[".", ":!xtask/src/showcase_flag.rs"],
    )?;
    if !hits.is_empty() {
        report.note("the retired showcase flag ALLODIA_DEMO is still referenced:");
        for file in hits.files() {
            report.note(format!("  {file}"));
        }
        report.note(
            "The flag is MAILCAL_SHOWCASE. A stale name here reads as a working guard that never \
             fires.",
        );
    }
    Ok(())
}

/// The Windows launcher names the mailbox it is about to open. If it parses a different variable
/// than the app does, it will cheerfully announce "your stored accounts" while opening the showcase
/// dataset, or, far worse, stay silent while opening the real one.
fn launcher_agrees(root: &Path, report: &mut Report) -> Result<(), String> {
    for file in [
        "clients/windows/build-and-run.ps1",
        "clients/windows/Mailcal/Services/ShowcaseMode.cs",
    ] {
        if !git::contains(root, "MAILCAL_SHOWCASE", file)? {
            report.note(format!("{file} no longer references MAILCAL_SHOWCASE."));
            report.note(
                "The launcher banner and the app must agree on the flag, or the banner lies.",
            );
        }
    }
    Ok(())
}

/// Showcase mode stays compiled out of a release build, on both platforms that could otherwise
/// honour a stray environment flag in a shipped binary.
fn release_gated(root: &Path, report: &mut Report) -> Result<(), String> {
    let cs = "clients/windows/Mailcal/Services/ShowcaseMode.cs";
    if !git::contains(root, "#if DEBUG", cs)? {
        report.note(format!("{cs} has no `#if DEBUG` guard."));
        report.note(
            "ShowcaseMode.IsOn must be hard-false in Release, or a shipped build honours \
             MAILCAL_SHOWCASE.",
        );
    }

    let guard = "#![cfg(any(debug_assertions, feature = \"dev-harness\"))]";
    if !git::contains(root, guard, LINUX_SHOWCASE)? {
        report.note(format!(
            "{LINUX_SHOWCASE} has no file-level `{guard}` guard."
        ));
        report.note("Showcase may exist only in debug or an explicit dev-harness build.");
    }

    // The Flatpak builds default features, so dev-harness must remain opt-in.
    let linux_cargo = "clients/linux/Cargo.toml";
    if features_default_enables(&git::read(root, linux_cargo)?, "dev-harness") {
        report.note(format!("{linux_cargo} enables dev-harness by default."));
        report.note("The Flatpak builds default features, so dev-harness must remain opt-in.");
    }

    // The committed manifest is the neutral one, named after the neutral app id; the packaging
    // script derives the branded copy beside it and removes it again. Read that id straight from
    // the committed defaults, so a checkout that has a brand is not sent looking for a name only a
    // package run ever writes.
    let env = git::read(root, "branding/default.env")?;
    let neutral_id = env
        .lines()
        .filter_map(|l| l.trim_end().strip_prefix("MAILCAL_APP_ID="))
        .map(|v| v.trim_matches('"').to_owned())
        .next_back()
        .unwrap_or_default();
    let manifest = format!("clients/linux/flatpak/{neutral_id}.yml");
    match git::read(root, &manifest) {
        Err(_) => {
            report.note(format!("{manifest} does not exist."));
            report.note(
                "The Flatpak manifest is named after MAILCAL_APP_ID in branding/default.env.",
            );
        }
        Ok(text) => {
            let builds_default = text.lines().any(|l| {
                l.trim().trim_start_matches('-').trim()
                    == "cargo build --release --locked -p mailcal-linux"
            });
            if !builds_default {
                report.note(format!(
                    "{manifest} no longer builds the exact default-feature Linux release."
                ));
                report
                    .note("The shipping command must not enable dev-harness or any showcase path.");
            }
        }
    }
    Ok(())
}

/// Every capture run proves the app is on the fictional dataset by finding one logged line. Reword
/// the emitting side and the matchers go blind; silently fixing one matcher and not the other
/// leaves a platform asserting nothing at all. Pin all three together.
///
/// The emitting side is searched as a *directory*: it used to be named as one file, and when that
/// file was split the marker moved and this check began failing on every run. A guard that cannot
/// pass is as useless as one that cannot fail.
fn marker_in_step(root: &Path, report: &mut Report) -> Result<(), String> {
    for file in [
        "crates/mailcal-bindings/src",
        "scripts/dev/lib.sh",
        SHOWCASE_PS1,
    ] {
        if !git::contains(root, MARKER, file)? {
            report.note(format!(
                "{file} no longer carries the showcase log marker:\n  {MARKER}"
            ));
            report.note(
                "Every capture path matches this line to prove the app is on the fictional \
                 dataset. All three copies must agree, or a platform stops checking.",
            );
        }
    }
    Ok(())
}

/// Compare the lists, not the sentence around them.
///
/// Pinning the marker's stable prefix stayed green while one driver still capped its locales at two
/// and the catalog grew to seven. A check that pins the part which cannot drift, while the part
/// that does drift goes unwatched, is not a check.
fn same_locales(
    root: &Path,
    showcase_sh: &str,
    showcase_ps1: &str,
    report: &mut Report,
) -> Result<Vec<String>, String> {
    let reference = lists::keep(
        &git::array_words(showcase_sh, "ALL_LOCALES"),
        lists::is_locale,
    );
    let lib = git::read(root, "scripts/dev/lib.sh")?;
    let from_lib = lists::keep(
        &lists::case_arm_labels(git::function_body(&lib, "showcase_marker_for")),
        lists::is_locale,
    );
    let from_ps = lists::keep(
        &lists::validate_set(showcase_ps1, "$Locale"),
        lists::is_locale,
    );

    for (file, other) in [("scripts/dev/lib.sh", &from_lib), (SHOWCASE_PS1, &from_ps)] {
        if *other != reference {
            report.note(format!(
                "{file} offers different showcase locales than {SHOWCASE_SH}:"
            ));
            report.note(format!("  {SHOWCASE_SH} : {}", lists::render(&reference)));
            report.note(format!("  {file}: {}", lists::render(other)));
            report.note(
                "Every capture path must offer the same languages, or a locale silently has no \
                 screenshots while the README says it does.",
            );
        }
    }
    if reference.is_empty() {
        report.note(format!(
            "parsed no locales out of {SHOWCASE_SH} (ALL_LOCALES): this check is blind."
        ));
    }
    Ok(reference)
}

/// A screen name is a cross-client contract, kept twice. Adding one to a single driver does not
/// fail loudly: the other dies on a parameter-binding error that reads like a broken script, and a
/// full run silently captures one screen fewer than every other platform.
fn same_screens(showcase_sh: &str, showcase_ps1: &str, report: &mut Report) -> Vec<String> {
    let mut reference = git::array_words(showcase_sh, "ALL_SCREENS");
    reference.extend(git::array_words(showcase_sh, "EXTRA_SCREENS"));
    reference.sort();
    reference.dedup();
    let reference = lists::keep(&reference, lists::is_screen_name);
    let from_ps = lists::keep(
        &lists::validate_set(showcase_ps1, "$Screen"),
        lists::is_screen_name,
    );

    if reference != from_ps {
        report.note(format!(
            "{SHOWCASE_PS1} offers different showcase screens than {SHOWCASE_SH}:"
        ));
        report.note(format!("  {SHOWCASE_SH} : {}", lists::render(&reference)));
        report.note(format!("  {SHOWCASE_PS1}: {}", lists::render(&from_ps)));
        report.note(
            "Adding a screen means editing ALL_SCREENS (or EXTRA_SCREENS) and the -Screen \
             ValidateSet.",
        );
    }
    if reference.is_empty() {
        report.note(format!(
            "parsed no screens out of {SHOWCASE_SH} (ALL_SCREENS): this check is blind."
        ));
    }
    reference
}

/// The capture loop names only the appearances it actually shoots; the driver's set also carries a
/// "however this desktop is set" option, which is a legitimate hand-run. So the rule is
/// containment, not equality: every word the loop can pass must be one the driver accepts.
fn same_appearances(
    showcase_sh: &str,
    showcase_ps1: &str,
    report: &mut Report,
) -> (Vec<String>, Vec<String>) {
    let shot = lists::appearances(showcase_sh);
    let accepted = lists::validate_set(showcase_ps1, "$Appearance");

    for appearance in &shot {
        if !accepted.contains(appearance) {
            report.note(format!(
                "{SHOWCASE_SH} captures in appearance \"{appearance}\", which {SHOWCASE_PS1} \
                 -Appearance does not accept ({}).",
                lists::render(&accepted)
            ));
            report.note("A Windows run would die on a parameter-binding error mid-capture.");
        }
    }
    if shot.is_empty() || accepted.is_empty() {
        report.note("parsed no appearances out of the capture scripts: this check is blind.");
    }
    (shot, accepted)
}

/// A client that does not recognise a screen name does not error: it falls back to the mailbox
/// list. So the run would enter showcase mode, shoot a clean, well-lit frame of the *inbox*, and
/// file it under the documentation screenshot's id. The first thing to notice would be a reader
/// following a setup guide illustrated with a picture of an inbox.
fn doc_screens_reachable(
    root: &Path,
    showcase_sh: &str,
    report: &mut Report,
) -> Result<(Vec<String>, Vec<String>), String> {
    let shared = lists::keep(
        &git::array_words(showcase_sh, "DOC_SCREENS_SHARED"),
        lists::is_screen_name,
    );
    let macos = lists::keep(
        &git::array_words(showcase_sh, "DOC_SCREENS_MACOS_ONLY"),
        lists::is_screen_name,
    );
    if shared.is_empty() || macos.is_empty() {
        report.note(format!(
            "parsed no documentation screens out of {SHOWCASE_SH}: this check is blind."
        ));
        report.note(
            "Expected DOC_SCREENS_SHARED and DOC_SCREENS_MACOS_ONLY (see docs/user-docs.md).",
        );
    }

    let apple = "clients/apple/Packages/MailcalKit/Sources/MailcalUI/ShowcaseMode.swift";
    let android = "clients/android/app/src/main/java/eu/allodia/mailcal/ShowcaseMode.kt";
    let apple_wanted: Vec<String> = shared.iter().chain(macos.iter()).cloned().collect();
    for (file, wanted) in [(apple, apple_wanted), (android, shared.clone())] {
        for screen in &wanted {
            if !git::contains(root, &format!("\"{screen}\""), file)? {
                report.note(format!(
                    "{file} cannot drive to the documentation screen \"{screen}\"."
                ));
                report.note(
                    "The run would fall back to the mailbox list and file a photograph of the \
                     INBOX under that screenshot id. Nothing downstream can tell the difference.",
                );
            }
        }
    }
    Ok((shared, macos))
}

/// Every store screen the capture driver offers Linux is one its client can actually reach.
///
/// Adding a name without a surface behind it breaks a whole capture run: the client refuses the
/// name and exits, which is loud, but it is loud in the middle of a 35-shot run rather than here.
fn linux_screens_reachable(
    root: &Path,
    showcase_sh: &str,
    report: &mut Report,
) -> Result<(), String> {
    let offered = lists::linux_store_screens(showcase_sh);
    if offered.is_empty() {
        report.note(format!(
            "parsed no Linux store screens out of {SHOWCASE_SH}: this check is blind."
        ));
        report.note("Expected a `linux)` arm in store_screens_for.");
        return Ok(());
    }
    let accepted = lists::accepted_screens(&git::read(root, LINUX_SHOWCASE)?);
    if accepted.is_empty() {
        report.note(format!(
            "parsed no accepted screens out of {LINUX_SHOWCASE}: this check is blind."
        ));
        report.note("Expected `Some(\"<name>\") => Ok(ShowcaseScreen::…)` arms in parse_screen.");
        return Ok(());
    }
    for screen in &offered {
        if !accepted.contains(screen) {
            report.note(format!(
                "{LINUX_SHOWCASE} does not accept the store screen \"{screen}\"."
            ));
            report.note(format!("  offered : {}", lists::render(&offered)));
            report.note(format!("  accepted: {}", lists::render(&accepted)));
        }
    }
    Ok(())
}

/// A direct device-theme read asks what the *phone* is set to, which is exactly the thing an
/// app-level Light or Dark overrides.
///
/// It is guarded here because the showcase is where it does visible damage: a capture pinned to
/// dark on a light emulator paints the app dark and then colours those swatches for a light device.
/// The frame is the right screen, in the right language, at the right pixel size, and it ships to a
/// store looking like a rendering bug.
fn no_device_theme(root: &Path, report: &mut Report) -> Result<(), String> {
    let theme = "clients/android/app/src/main/java/eu/allodia/mailcal/Theme.kt";
    let hits = git::grep(
        root,
        Kind::Fixed,
        "isSystemInDarkTheme",
        &["clients/android/app/src/main", &format!(":!{theme}")],
    )?;
    if !hits.is_empty() {
        report.note("these Android sources read the DEVICE theme directly:");
        for file in hits.files() {
            report.note(format!("  {file}"));
        }
        report.note("Read LocalAppDark instead (Theme.kt).");
    }
    Ok(())
}

/// True when the `[features]` table's `default` list mentions `feature`.
fn features_default_enables(manifest: &str, feature: &str) -> bool {
    let mut inside = false;
    for line in manifest.lines() {
        let trimmed = line.trim_end();
        if trimmed.starts_with("[features]") {
            inside = true;
            continue;
        }
        if trimmed.starts_with('[') {
            inside = false;
        }
        if inside && trimmed.trim_start().starts_with("default") && trimmed.contains(feature) {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::features_default_enables;

    #[test]
    fn spots_a_default_enabled_feature() {
        let manifest = "[features]\ndefault = [\"dev-harness\"]\n";
        assert!(features_default_enables(manifest, "dev-harness"));
    }

    #[test]
    fn an_opt_in_feature_is_fine() {
        let manifest = "[features]\ndefault = []\ndev-harness = []\n";
        assert!(!features_default_enables(manifest, "dev-harness"));
    }

    #[test]
    fn a_default_outside_the_features_table_is_not_read() {
        // A `default` key in another table says nothing about the feature set.
        let manifest = "[features]\ndefault = []\n\n[other]\ndefault = \"dev-harness\"\n";
        assert!(!features_default_enables(manifest, "dev-harness"));
    }
}
