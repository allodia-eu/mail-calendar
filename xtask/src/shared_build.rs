//! Making a gate run report on the checkout it was started from.
//!
//! `.cargo/config.toml` points `build.build-dir` at one directory every checkout shares, so the
//! dependency compile is paid for once rather than once per worktree. Cargo cannot tell those
//! checkouts apart: it names an artifact, and the fingerprint guarding it, from the
//! workspace-*relative* path, so they all land on the same `deps/<crate>-<hash>` and the same
//! fingerprint entry, and freshness then comes down to mtimes against a relative file list. A
//! checkout whose sources predate the build another checkout last ran is declared fresh and is
//! handed that checkout's binary. A suite then reports `1 passed` for an assertion this tree
//! cannot satisfy, which is exactly what a freshly created worktree is in line for: its files are
//! the newest thing that has not been compiled yet.
//!
//! So a gate run claims the directory. It leaves a marker naming the checkout whose artifacts are
//! in there, and when the marker names someone else it runs `cargo clean --workspace` first, which
//! drops the **members'** artifacts and leaves every dependency compiled. That is the whole of the
//! sharing: the members collide, and sharing them was never buying anything, because two checkouts
//! cannot both have their build in one entry.
//!
//! It costs a rebuild of this repository's own crates, and only where the answer would otherwise
//! have been wrong: a checkout that built here last claims it again for nothing.
//!
//! ⚠️ This covers the **gate**, which is what has to be true before a push. A `cargo test` typed
//! by hand still gets whatever the shared directory holds. AGENTS.md and `docs/debugging.md` carry
//! that, and the tell: a result that cannot be squared with the diff.

use std::{path::Path, process::Command};

use crate::prune::build_directory;

/// The file naming the checkout whose artifacts are in the shared build directory.
const MARKER: &str = ".mailcal-checkout";

/// What a run has to do about the build directory it is about to use.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Claim {
    /// Nothing to do: the build directory belongs to this checkout alone, or cargo would not say
    /// where it is, which is the same answer since there is nothing to act on.
    Private,
    /// This checkout built here last, so what is in there is its own.
    Held,
    /// Someone else built here last, or nobody has claimed it since it was made. Either way what
    /// is in there is not this checkout's to trust.
    Taken,
}

/// Claims the shared build directory for `root`, rebuilding the workspace if it was someone
/// else's.
///
/// Best effort by design: every failure here leaves the gate running, because a gate that refuses
/// to start over its own bookkeeping is worse than the reuse it is guarding against.
pub(crate) fn claim(root: &Path) {
    let Some(build_dir) = build_directory(root) else {
        return;
    };
    if decide(root, &build_dir) != Claim::Taken {
        return;
    }
    println!(
        "== the shared build directory last held another checkout's build, so this repository's \
         own\n   crates are rebuilt here before anything is judged ({})",
        build_dir.display()
    );
    let cleaned = Command::new("cargo")
        .args(["clean", "--workspace", "--quiet"])
        .current_dir(root)
        .status()
        .is_ok_and(|status| status.success());
    if !cleaned {
        eprintln!(
            "!! could not clean the workspace: the steps below may report on another checkout's \
             build"
        );
        return;
    }
    // Written only once the clean succeeded, so a failure is claimed by nobody and the next run
    // tries again rather than trusting what this one could not replace.
    if std::fs::write(build_dir.join(MARKER), root.to_string_lossy().as_bytes()).is_err() {
        eprintln!(
            "!! could not record this checkout in {}",
            build_dir.display()
        );
    }
}

/// Whether the build directory at `build_dir` is this checkout's to trust.
fn decide(root: &Path, build_dir: &Path) -> Claim {
    if build_dir.starts_with(root) {
        return Claim::Private;
    }
    match std::fs::read_to_string(build_dir.join(MARKER)) {
        Ok(held) if Path::new(held.trim()) == root => Claim::Held,
        _ => Claim::Taken,
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{Claim, MARKER, decide};

    /// A directory nobody else writes to, named for the test using it.
    fn scratch(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("mailcal-shared-build-{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("a scratch directory");
        dir
    }

    #[test]
    fn a_build_directory_inside_the_checkout_is_nobody_else_s() {
        let root = scratch("private");
        assert_eq!(decide(&root, &root.join("target")), Claim::Private);
    }

    #[test]
    fn an_unclaimed_directory_is_taken() {
        // Nobody has said whose the artifacts are, so they are not this checkout's to trust: a
        // shared directory that predates the marker is exactly the case this exists for.
        let build = scratch("unclaimed");
        assert_eq!(decide(Path::new("/some/checkout"), &build), Claim::Taken);
    }

    #[test]
    fn a_marker_naming_this_checkout_is_held() {
        let build = scratch("held");
        std::fs::write(build.join(MARKER), "/some/checkout").expect("the marker writes");
        assert_eq!(decide(Path::new("/some/checkout"), &build), Claim::Held);
    }

    #[test]
    fn a_marker_naming_another_checkout_is_taken() {
        let build = scratch("taken");
        std::fs::write(build.join(MARKER), "/another/checkout").expect("the marker writes");
        assert_eq!(decide(Path::new("/some/checkout"), &build), Claim::Taken);
    }

    #[test]
    fn a_trailing_newline_in_the_marker_still_names_the_checkout() {
        // Written without one, but a marker is a plain text file in a directory people poke at.
        let build = scratch("newline");
        std::fs::write(build.join(MARKER), "/some/checkout\n").expect("the marker writes");
        assert_eq!(decide(Path::new("/some/checkout"), &build), Claim::Held);
    }
}
