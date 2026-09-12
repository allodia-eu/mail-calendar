//! Fails when the default build can reach into `allodia_license/`.
//!
//! That directory is the one part of the repository under a licence other than the GPL, and the
//! promise attached to it is that the open tree does not need it: the application compiles, tests
//! and runs with no reference to anything closed. A promise nothing checks is a promise that
//! quietly stops being true the first time an import looks convenient.

use std::path::Path;

use crate::git::{self, Kind};

/// The directory under the other licence.
const DIR: &str = "allodia_license";

/// Where `reuse lint` reads the licence text, which must match the copy a person looks at.
const MIRROR: &str = "LICENSES/LicenseRef-Allodia-1.0.txt";

/// Runs the check. `Ok(true)` means the default build stands alone.
///
/// # Errors
///
/// Propagates a git or filesystem failure.
pub(crate) fn run(root: &Path) -> Result<bool, String> {
    let mut failed = false;

    failed |= !outside_the_default_build(root)?;
    failed |= !nothing_reaches_in(root)?;
    failed |= !the_feature_is_off(root)?;
    failed |= !the_two_texts_agree(root);

    if failed {
        return Ok(false);
    }
    println!("OK: the default build does not reach into {DIR}/.");
    Ok(true)
}

/// The crate is a workspace *member* on purpose: that is what lets it share one lockfile, one set
/// of dependency versions and one copy of the engine. What keeps it out of a shipped build is its
/// absence from `default-members`, which is an explicit list rather than a subtraction, so adding a
/// member never adds it to the default build.
fn outside_the_default_build(root: &Path) -> Result<bool, String> {
    let manifest = git::read(root, "Cargo.toml")?;
    let Some(list) = default_members(&manifest) else {
        eprintln!(
            "ERROR: Cargo.toml has no default-members list, so every member is in the default \
             build."
        );
        return Ok(false);
    };
    if list.contains(DIR) {
        eprintln!("ERROR: {DIR}/ is in default-members, so a bare `cargo build` ships it.");
        return Ok(false);
    }
    Ok(true)
}

/// The **path**, not the word.
///
/// `allodia_license` is also the Rust identifier of the crate that lives there, and
/// `use allodia_license::…` is not a reach into a directory; whether that crate is linked at all is
/// decided by the feature check below. What a reach looks like is a path with a component after it:
/// a `path =` in a manifest, a `srcDir(…)`, an `#[path]`.
///
/// Only build inputs are searched. Prose *should* name the directory, and a rule that forbade the
/// word would be a rule nobody could document the seam under.
fn nothing_reaches_in(root: &Path) -> Result<bool, String> {
    let hits = git::grep(
        root,
        Kind::Fixed,
        &format!("{DIR}/"),
        &[
            "crates",
            "clients",
            "Cargo.toml",
            ":!clients/*/README.md",
            ":!clients/*/*/README.md",
        ],
    )?
    .without(is_the_members_line)
    .without(is_the_optional_dependency);

    if !hits.is_empty() {
        eprintln!("ERROR: the default build references {DIR}/ (it must build without it):");
        for line in &hits.lines {
            eprintln!("{line}");
        }
        return Ok(false);
    }
    Ok(true)
}

/// The optional dependency is only half the guarantee. A feature listed in `default = [...]` is on
/// for everyone, and the manifest would still read as "optional" to anyone skimming it.
fn the_feature_is_off(root: &Path) -> Result<bool, String> {
    let hits = git::grep(
        root,
        Kind::Extended,
        "^default = .*allodia-license",
        &["crates"],
    )?;
    if !hits.is_empty() {
        eprintln!("ERROR: the {DIR} feature is on by default, so every build links it:");
        for line in &hits.lines {
            eprintln!("{line}");
        }
        return Ok(false);
    }
    Ok(true)
}

/// The text lives twice for the same reason the GPL does: `LICENSES/` is where `reuse lint` reads a
/// licence, and the directory itself is where a person looks. A copy nobody compares is a copy that
/// disagrees, and `reuse lint` checks that the text exists, never that it says the same thing.
fn the_two_texts_agree(root: &Path) -> bool {
    let source = root.join(DIR).join("LICENSE.md");
    let mirror = root.join(MIRROR);
    match (std::fs::read(&source), std::fs::read(&mirror)) {
        (Ok(a), Ok(b)) if a == b => true,
        (Ok(_), Ok(_)) => {
            eprintln!(
                "ERROR: {DIR}/LICENSE.md and {MIRROR} have drifted. Copy one over the other."
            );
            false
        }
        (Err(_), Err(_)) => true,
        _ => {
            eprintln!("ERROR: {DIR}/LICENSE.md and {MIRROR} must both exist: one is missing.");
            false
        }
    }
}

/// The `default-members = [ … ]` block, as written.
fn default_members(manifest: &str) -> Option<String> {
    let start = manifest.find("default-members")?;
    let rest = &manifest[start..];
    let end = rest.find(']')?;
    Some(rest[..=end].to_owned())
}

/// The `members` line is the one reference a manifest must carry, and the comment above it says
/// why, which is worth more to the next reader than a clean search.
fn is_the_members_line(hit: &str) -> bool {
    let Some((path, _, content)) = split_hit(hit) else {
        return false;
    };
    path == "Cargo.toml" && {
        let t = content.trim_start();
        t.starts_with('#') || t.starts_with("members")
    }
}

/// The seam itself: the line has to exist for a branded build to turn the directory on, and it is
/// `optional = true` that keeps it out of everyone else's.
///
/// Which crate carries it is deliberately not pinned: the seam has moved once already, and a rule
/// about *shape* that fails over a move changing nothing about that shape is a rule people learn to
/// edit rather than to read. What is pinned is the shape.
fn is_the_optional_dependency(hit: &str) -> bool {
    let Some((path, _, content)) = split_hit(hit) else {
        return false;
    };
    let is_crate_manifest = path.starts_with("crates/")
        && path.ends_with("/Cargo.toml")
        && path.matches('/').count() == 2;
    if !is_crate_manifest {
        return false;
    }
    let t = content.trim_start();
    t.starts_with('#')
        || t == format!(
            "allodia-license = {{ path = \"../../{DIR}/crates/allodia-license\", optional = true }}"
        )
        || t == "allodia-license = [\"dep:allodia-license\"]"
}

/// Splits a `path:line:content` hit.
fn split_hit(hit: &str) -> Option<(&str, &str, &str)> {
    let (path, rest) = hit.split_once(':')?;
    let (number, content) = rest.split_once(':')?;
    Some((path, number, content))
}

#[cfg(test)]
mod tests {
    use super::{default_members, is_the_members_line, is_the_optional_dependency, split_hit};

    #[test]
    fn reads_the_default_members_block() {
        let manifest = "members = [\"a\"]\ndefault-members = [\n  \"crates/x\",\n]\n";
        let list = default_members(manifest).unwrap();
        assert!(list.contains("crates/x"));
        // The `members` line above must not be swept in, or the check reads the wrong list.
        assert!(!list.contains("\"a\""));
    }

    #[test]
    fn the_members_line_is_allowed_but_only_in_the_root_manifest() {
        assert!(is_the_members_line(
            "Cargo.toml:6:members = [\"crates/*\", \"allodia_license/x\"]"
        ));
        assert!(!is_the_members_line(
            "crates/x/Cargo.toml:6:members = [\"allodia_license/x\"]"
        ));
    }

    #[test]
    fn the_seam_is_allowed_in_any_crate_manifest() {
        let line = "crates/mailcal-bindings/Cargo.toml:40:allodia-license = { path = \
                    \"../../allodia_license/crates/allodia-license\", optional = true }";
        assert!(is_the_optional_dependency(line));
    }

    #[test]
    fn a_non_optional_dependency_is_not_the_seam() {
        let line = "crates/mailcal-bindings/Cargo.toml:40:allodia-license = { path = \
                    \"../../allodia_license/crates/allodia-license\" }";
        assert!(!is_the_optional_dependency(line));
    }

    #[test]
    fn splits_a_hit_with_colons_in_the_content() {
        let (path, number, content) = split_hit("a/b.rs:12:let x = y::z;").unwrap();
        assert_eq!((path, number), ("a/b.rs", "12"));
        assert_eq!(content, "let x = y::z;");
    }
}
