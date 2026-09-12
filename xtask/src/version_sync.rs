//! Fails when the one app version has drifted between its source of truth and any of its mirrors.
//!
//! The top-level `/VERSION` file is the single marketing version (`docs/versioning.md`). Two build
//! systems cannot read an external file, so they carry a committed mirror this check pins to it:
//! Cargo's `[workspace.package] version`, and `project.yml`'s `MARKETING_VERSION`. `Cargo.lock` is
//! a third in all but name.
//!
//! The other three clients derive their version from `/VERSION` at build time, so there is nothing
//! to drift, but a hardcoded literal creeping back is exactly the regression that would silently
//! un-sync them, so each is also asserted still to read the file.

use std::path::Path;

use crate::{git, report::Report};

/// Runs the check. `Ok(true)` means every mirror agrees with `/VERSION`.
///
/// # Errors
///
/// Fails when `/VERSION` is missing or malformed: there is nothing to compare against.
pub(crate) fn run(root: &Path) -> Result<bool, String> {
    let mut report = Report::new();

    let raw = git::read(root, "VERSION")
        .map_err(|_| "/VERSION is missing: it is the single source of truth.".to_owned())?;
    let version: String = raw.chars().filter(|c| !c.is_whitespace()).collect();
    let Some(parts) = parse_triple(&version) else {
        return Err(format!(
            "/VERSION must be a single MAJOR.MINOR.PATCH line (got '{version}')."
        ));
    };

    committed_mirrors(root, &version, &mut report)?;
    derived_mechanisms(root, &mut report)?;
    release_note(root, &version, parts, &mut report);

    if report.failed() {
        report.emit();
        eprintln!(
            "ERROR: app version is out of sync: see above. Bump with scripts/dev/bump-version.sh, \
             or fix the drift."
        );
        return Ok(false);
    }

    println!("OK: every version mirror and derivation is in sync with /VERSION ({version}).");
    Ok(true)
}

/// The three committed copies, which must equal `/VERSION` exactly.
fn committed_mirrors(root: &Path, version: &str, report: &mut Report) -> Result<(), String> {
    let cargo = git::read(root, "Cargo.toml")?;
    let found = workspace_package_version(&cargo);
    if found.as_deref() != Some(version) {
        report.note(format!(
            "Cargo.toml [workspace.package] version is '{}', expected '{version}'.",
            found.unwrap_or_default()
        ));
    }

    // The lockfile is the mirror that used to be missed. It carries a version per workspace member,
    // and a release that moved Cargo.toml without it produced a tag whose every `--locked` build
    // failed with "cannot update the lock file". Checked against one member that is always present
    // rather than all of them, because one drifting means the bump skipped the lock entirely.
    let lock = git::read(root, "Cargo.lock")?;
    let found = locked_version(&lock, "mailcal-app");
    if found.as_deref() != Some(version) {
        report.note(format!(
            "Cargo.lock has mailcal-app at '{}', expected '{version}'. Run: cargo update \
             --workspace",
            found.unwrap_or_default()
        ));
    }

    let project = git::read(root, "clients/apple/project.yml")?;
    let found = marketing_version(&project);
    if found.as_deref() != Some(version) {
        report.note(format!(
            "clients/apple/project.yml MARKETING_VERSION is '{}', expected '{version}'.",
            found.unwrap_or_default()
        ));
    }
    Ok(())
}

/// The clients that derive their version, which must still read the file.
fn derived_mechanisms(root: &Path, report: &mut Report) -> Result<(), String> {
    let gradle_path = "clients/android/app/build.gradle.kts";
    let gradle = git::read(root, gradle_path)?;
    if !gradle.contains(r#"rootProject.file("../../VERSION")"#) {
        report.note(format!(
            "{gradle_path} no longer reads ../../VERSION: the version must stay derived."
        ));
    }
    if assigns_numeric_literal(&gradle, "versionName") {
        report.note(format!(
            "{gradle_path} has a hardcoded versionName literal: derive it from /VERSION instead."
        ));
    }

    let csproj_path = "clients/windows/Mailcal/Mailcal.csproj";
    let csproj = git::read(root, csproj_path)?;
    if !csproj
        .lines()
        .any(|l| l.contains("ReadAllText") && l.contains("VERSION"))
    {
        report.note(format!(
            "{csproj_path} no longer reads /VERSION for <Version>: DeviceFacts.cs would report a \
             stale app_version."
        ));
    }

    let pkgsh_path = "clients/apple/Scripts/package.sh";
    let pkgsh = git::read(root, pkgsh_path)?;
    if !pkgsh.contains(r#"cat "$ROOT/VERSION""#) {
        report.note(format!(
            "{pkgsh_path} no longer defaults MARKETING_VERSION from /VERSION."
        ));
    }
    if !pkgsh.contains("date -u +%Y.%m%d.%H%M") {
        report.note(format!(
            "{pkgsh_path} build number is not the dotted timestamp (date -u +%Y.%m%d.%H%M): a \
             single integer overflows CFBundleVersion."
        ));
    }
    if pkgsh.contains("date +%Y%m%d%H%M") {
        report.note(format!(
            "{pkgsh_path} still has the overflowing single-integer build number \
             (date +%Y%m%d%H%M)."
        ));
    }

    // The Linux metainfo is generated at build time. The regression guarded here is a committed
    // literal: a `<release version="0.4.0" …/>` typed into the template would build, install and
    // validate, then advertise whatever version its last editor happened to have.
    let template_path = "clients/linux/flatpak/metainfo.xml.in";
    let generator_path = "scripts/dev/flatpak_metadata.py";
    if has_release_version_literal(&git::read(root, template_path)?) {
        report.note(format!(
            "{template_path} has a hardcoded <release version=…> literal: the release list is \
             generated by {generator_path} from /VERSION."
        ));
    }
    if !git::read(root, generator_path)?.contains("VERSION") {
        report.note(format!(
            "{generator_path} no longer reads /VERSION: the Linux client's advertised version must \
             stay derived."
        ));
    }

    let pkgps_path = "clients/windows/package.ps1";
    let pkgps = git::read(root, pkgps_path)?;
    if !pkgps.contains("Get-Content (Join-Path $root 'VERSION')") {
        report.note(format!(
            "{pkgps_path} no longer reads /VERSION for the package version."
        ));
    }
    // Reading /VERSION is not enough: the sideload path once ignored it and hardcoded a
    // "1.0.$build.$rev", with major.minor stuck at 1.0 regardless. Guard the shape directly: any
    // literal assigned to $Version must not begin with a digit.
    if assigns_numeric_literal(&pkgps, "$Version") {
        report.note(format!(
            "{pkgps_path} assigns a hardcoded numeric $Version literal: derive MAJOR.MINOR from \
             /VERSION ($semver) instead."
        ));
    }
    Ok(())
}

/// The release note for this version, which must exist and must not be out-run by a newer one.
///
/// `/VERSION` means "the version users currently have", so it can only name a release that was
/// actually assembled. A version with no note is a release nobody can describe; a note above
/// `/VERSION` is a release that has not happened.
fn release_note(root: &Path, version: &str, parts: [u64; 3], report: &mut Report) {
    let dir = "docs/changelog/released";
    if !root.join(dir).join(format!("{version}.md")).is_file() {
        report.note(format!(
            "{dir}/{version}.md is missing: /VERSION names a release with no note. Cut the release \
             with scripts/dev/release.py rather than editing /VERSION by hand."
        ));
    }

    let Ok(entries) = std::fs::read_dir(root.join(dir)) else {
        return;
    };
    let mut names: Vec<String> = entries
        .filter_map(Result::ok)
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|n| {
            Path::new(n)
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("md"))
        })
        .collect();
    names.sort();

    for name in names {
        let stem = name.trim_end_matches(".md");
        let Some(candidate) = parse_triple(stem) else {
            report.note(format!(
                "{dir}/{name} is not named X.Y.Z.md: the filename is how /VERSION finds its note."
            ));
            continue;
        };
        if candidate > parts {
            report.note(format!(
                "{dir}/{name} names {stem}, which is above /VERSION ({version}): a released note \
                 may not describe a release that has not happened."
            ));
        }
    }
}

/// Three plain integers, or `None`.
///
/// Compared as integers rather than with `sort -V`, whose prerelease handling differs between
/// implementations, and these are always three plain numbers.
fn parse_triple(text: &str) -> Option<[u64; 3]> {
    let mut parts = text.split('.');
    let triple = [
        parts.next()?.parse().ok()?,
        parts.next()?.parse().ok()?,
        parts.next()?.parse().ok()?,
    ];
    if parts.next().is_some() {
        return None;
    }
    Some(triple)
}

/// The `version` line inside `[workspace.package]`, not a dependency's inline version.
fn workspace_package_version(manifest: &str) -> Option<String> {
    let mut inside = false;
    for line in manifest.lines() {
        let trimmed = line.trim_end();
        if trimmed.starts_with("[workspace.package]") {
            inside = true;
            continue;
        }
        if trimmed.starts_with('[') {
            inside = false;
        }
        if inside && trimmed.starts_with("version") {
            return Some(digits_and_dots(trimmed));
        }
    }
    None
}

/// The `version` a lockfile records for one package.
fn locked_version(lock: &str, package: &str) -> Option<String> {
    let header = format!("name = \"{package}\"");
    let mut found = false;
    for line in lock.lines() {
        let trimmed = line.trim_end();
        if trimmed == header {
            found = true;
            continue;
        }
        if found && trimmed.starts_with("version = \"") {
            return Some(digits_and_dots(trimmed));
        }
    }
    None
}

/// `MARKETING_VERSION: "x.y.z"` under `settings.base`.
///
/// The `$(MARKETING_VERSION)` Info.plist reference has no trailing colon, so this matches only the
/// real setting.
fn marketing_version(project: &str) -> Option<String> {
    for line in project.lines() {
        let Some(rest) = line.split_once("MARKETING_VERSION:") else {
            continue;
        };
        let value: String = rest
            .1
            .trim()
            .trim_matches('"')
            .chars()
            .take_while(|c| c.is_ascii_digit() || *c == '.')
            .collect();
        if parse_triple(&value).is_some() {
            return Some(value);
        }
    }
    None
}

/// Everything but the digits and dots removed, which is what the shell original's `gsub` did.
fn digits_and_dots(line: &str) -> String {
    line.chars()
        .filter(|c| c.is_ascii_digit() || *c == '.')
        .collect()
}

/// True when `name` is assigned a string literal starting with a digit.
fn assigns_numeric_literal(haystack: &str, name: &str) -> bool {
    haystack.lines().any(|line| {
        let Some(rest) = line.split_once(name) else {
            return false;
        };
        let after = rest.1.trim_start();
        let Some(after) = after.strip_prefix('=') else {
            return false;
        };
        after
            .trim_start()
            .strip_prefix('"')
            .is_some_and(|v| v.starts_with(|c: char| c.is_ascii_digit()))
    })
}

/// True when the metainfo template carries a `<release version="N…"` literal.
fn has_release_version_literal(template: &str) -> bool {
    template.lines().any(|line| {
        line.split_once("<release").is_some_and(|(_, rest)| {
            rest.trim_start()
                .strip_prefix("version=\"")
                .is_some_and(|v| v.starts_with(|c: char| c.is_ascii_digit()))
        })
    })
}

#[cfg(test)]
mod tests {
    use super::{
        assigns_numeric_literal, has_release_version_literal, locked_version, marketing_version,
        parse_triple, workspace_package_version,
    };

    #[test]
    fn reads_the_workspace_version_not_a_dependency() {
        let manifest =
            "[dependencies]\nversion = \"9.9.9\"\n\n[workspace.package]\nversion = \"0.5.0\"\n";
        assert_eq!(
            workspace_package_version(manifest).as_deref(),
            Some("0.5.0")
        );
    }

    #[test]
    fn a_section_ends_the_workspace_package_block() {
        let manifest =
            "[workspace.package]\nedition = \"2024\"\n\n[profile.dev]\nversion = \"1.2.3\"\n";
        assert_eq!(workspace_package_version(manifest), None);
    }

    #[test]
    fn reads_the_named_packages_locked_version() {
        let lock = "[[package]]\nname = \"other\"\nversion = \"9.9.9\"\n\n[[package]]\nname = \
                    \"mailcal-app\"\nversion = \"0.5.0\"\n";
        assert_eq!(
            locked_version(lock, "mailcal-app").as_deref(),
            Some("0.5.0")
        );
    }

    #[test]
    fn a_package_absent_from_the_lockfile_is_not_guessed_at() {
        // Returning None is what makes the caller report the drift rather than compare "" to "".
        assert_eq!(
            locked_version("[[package]]\nname = \"other\"\n", "gone"),
            None
        );
    }

    #[test]
    fn reads_marketing_version_but_not_the_plist_reference() {
        let project = "  INFOPLIST: $(MARKETING_VERSION)\n  MARKETING_VERSION: \"0.5.0\"\n";
        assert_eq!(marketing_version(project).as_deref(), Some("0.5.0"));
    }

    #[test]
    fn versions_compare_as_integers_not_strings() {
        // "0.10.0" sorts below "0.9.0" as text, and above it as a version.
        assert!(parse_triple("0.10.0").unwrap() > parse_triple("0.9.0").unwrap());
    }

    #[test]
    fn rejects_a_four_part_version() {
        assert_eq!(parse_triple("1.2.3.4"), None);
    }

    #[test]
    fn spots_a_hardcoded_literal_but_not_a_derived_one() {
        assert!(assigns_numeric_literal(
            "  versionName = \"1.0.0\"",
            "versionName"
        ));
        assert!(!assigns_numeric_literal(
            "  versionName = readVersion()",
            "versionName"
        ));
        assert!(!assigns_numeric_literal(
            "  $Version = \"$($mm[0]).0\"",
            "$Version"
        ));
    }

    #[test]
    fn spots_a_committed_release_literal() {
        assert!(has_release_version_literal(
            r#"  <release version="0.4.0" date="x"/>"#
        ));
        assert!(!has_release_version_literal(
            "  <!-- releases are generated -->"
        ));
    }
}
