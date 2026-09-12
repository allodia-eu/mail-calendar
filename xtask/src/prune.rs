//! Reclaiming the incremental build cache once a chunk of work is finished.
//!
//! The cache is pure cache, and nothing reclaims it. Cargo mints a session directory per distinct
//! compilation context: every engine re-pin, every toggle of the local-engine override, every
//! feature set, and keeps them all. `cargo clean` has no stale, age or size option, and the
//! collector nightly offers cleans `$CARGO_HOME`, not the build directory. Two days of one branch's
//! work left 20 GiB of it on one machine.
//!
//! So: drop it once a chunk of work is done and only when it is over the cap, since the cost is a
//! real one. Measured on an M-series Mac, `cargo build -p mailcal-bindings` after a one-line edit
//! in `mailcal-app`: 3.3s with the cache, 12.0s on the first rebuild after a prune, 3.4s from then
//! on. The cap is high enough that a normal branch never trips it.
//!
//! It runs at the end of a green gate because that is already the point where the next thing is a
//! push; a cleanup anybody has to *remember* is the same failure mode as a gate run from memory.

use std::{
    path::{Path, PathBuf},
    process::Command,
};

/// How much incremental cache is tolerated before a green gate drops it.
const CAP_GIB: u64 = 5;

/// How far below a root an `incremental` directory can sit.
///
/// Both `<root>/debug/incremental` and the per-triple `<root>/<triple>/debug/incremental`, which is
/// why the walk goes three levels down rather than one.
const MAX_DEPTH: u8 = 3;

/// Drops the incremental caches when they are over the cap, and says so.
pub(crate) fn incremental(root: &Path) {
    if let Some(gib) = prune_over(&roots(root), CAP_GIB) {
        println!(
            "\n== reclaimed {gib} GiB of incremental build cache (past the {CAP_GIB} GiB cap; your \
             next\n   rebuild after an edit costs about nine seconds more, once. See \
             docs/debugging.md)"
        );
    }
}

/// Where the caches live.
///
/// `.cargo/config.toml` points the intermediates outside the checkout so worktrees share the
/// compile work, so with a shared build directory `target/` holds no incremental cache at all and
/// pruning only it would silently reclaim nothing. Both are looked at, because the setting is a
/// config file anyone can opt out of.
fn roots(root: &Path) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    let target = root.join("target");
    if target.is_dir() {
        roots.push(target.clone());
    }
    if let Some(shared) = build_directory(root)
        && shared != target
        && shared.is_dir()
    {
        roots.push(shared);
    }
    roots
}

/// Cargo's build directory, as cargo itself reports it.
///
/// Asked rather than read out of the config file, because the setting takes placeholders and can be
/// overridden by an ancestor config or the environment. `None` when cargo cannot say, which is what
/// the prune's own tests see.
fn build_directory(root: &Path) -> Option<PathBuf> {
    let output = Command::new("cargo")
        .args(["metadata", "--format-version", "1", "--no-deps"])
        .current_dir(root)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    json_string_field(&String::from_utf8_lossy(&output.stdout), "build_directory")
        .map(PathBuf::from)
}

/// The value of a top-level string field in one line of JSON.
///
/// The escapes have to be undone: on Windows the path arrives as `D:\\Users\\…`, and a reader that
/// hands that back names a directory that does not exist, so the prune finds nothing and reports
/// nothing. A whole JSON parser to read one string is the wrong trade for a crate whose value is
/// that it compiles from nothing.
fn json_string_field(json: &str, field: &str) -> Option<String> {
    let needle = format!("\"{field}\":\"");
    let rest = &json[json.find(&needle)? + needle.len()..];
    let mut out = String::new();
    let mut chars = rest.chars();
    while let Some(c) = chars.next() {
        match c {
            '"' => return Some(out),
            '\\' => out.push(chars.next()?),
            _ => out.push(c),
        }
    }
    None
}

/// Removes every incremental cache under `roots` when together they exceed `cap_gib`, returning
/// what was reclaimed.
///
/// The cap is a parameter so the removal can be tested. What matters is not that it prunes (that
/// only costs disk) but *what survives*: this is the one place a recursive delete is driven by a
/// search, over the developer's whole build directory. A widened match would take `deps/` with it,
/// which is the rebuild the cache exists to avoid, or something outside the build tree entirely.
fn prune_over(roots: &[PathBuf], cap_gib: u64) -> Option<u64> {
    let mut dirs: Vec<PathBuf> = Vec::new();
    let mut total: u64 = 0;
    for root in roots {
        collect_incremental(root, 0, &mut dirs, &mut total);
    }
    if dirs.is_empty() {
        return None;
    }
    let gib = total / (1024 * 1024 * 1024);
    if gib < cap_gib {
        return None;
    }
    for dir in dirs {
        let _ = std::fs::remove_dir_all(dir);
    }
    Some(gib)
}

/// Finds every `incremental` directory near the top of a build tree and sums its size.
fn collect_incremental(dir: &Path, depth: u8, found: &mut Vec<PathBuf>, total: &mut u64) {
    if depth >= MAX_DEPTH {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        if entry.file_name() == "incremental" {
            *total += directory_size(&path);
            found.push(path);
        } else {
            collect_incremental(&path, depth + 1, found, total);
        }
    }
}

/// Bytes held under `dir`.
fn directory_size(dir: &Path) -> u64 {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return 0;
    };
    entries
        .filter_map(Result::ok)
        .map(|e| match e.metadata() {
            Ok(m) if m.is_dir() => directory_size(&e.path()),
            Ok(m) => m.len(),
            Err(_) => 0,
        })
        .sum()
}

#[cfg(test)]
mod tests {
    use super::{json_string_field, prune_over};
    use crate::fixture::Repo;

    /// A miniature build tree: two incremental caches, and the artefacts that must outlive them.
    fn tree() -> Repo {
        Repo::new(&[
            ("target/debug/incremental/a/one.bin", "x"),
            (
                "target/aarch64-pc-windows-msvc/debug/incremental/b/two.bin",
                "x",
            ),
            ("target/debug/deps/libthing.rlib", "keep me"),
            ("target/debug/build/script.exe", "keep me"),
            ("src/main.rs", "keep me"),
        ])
    }

    #[test]
    fn under_the_cap_nothing_is_touched() {
        let repo = tree();
        assert_eq!(prune_over(&[repo.root().join("target")], 1), None);
        assert!(
            repo.root()
                .join("target/debug/incremental/a/one.bin")
                .exists()
        );
    }

    #[test]
    fn over_the_cap_only_the_incremental_caches_go() {
        let repo = tree();
        assert!(prune_over(&[repo.root().join("target")], 0).is_some());
        assert!(!repo.root().join("target/debug/incremental").exists());
        assert!(
            !repo
                .root()
                .join("target/aarch64-pc-windows-msvc/debug/incremental")
                .exists()
        );
        // What survives is the point: deps/ is the rebuild the cache exists to avoid.
        assert!(repo.root().join("target/debug/deps/libthing.rlib").exists());
        assert!(repo.root().join("target/debug/build/script.exe").exists());
    }

    #[test]
    fn nothing_outside_the_named_root_is_reachable() {
        let repo = tree();
        prune_over(&[repo.root().join("target")], 0);
        assert!(repo.root().join("src/main.rs").exists());
    }

    #[test]
    fn a_tree_with_no_build_directory_is_not_an_error() {
        let repo = Repo::new(&[("src/main.rs", "x")]);
        assert_eq!(prune_over(&[repo.root().join("target")], 0), None);
    }

    #[test]
    fn a_windows_path_survives_its_json_escaping() {
        // Handed back unescaped, this names a directory that does not exist, and the prune then
        // reclaims nothing while reporting nothing.
        let json = r#"{"a":1,"build_directory":"D:\\Users\\x\\build","b":2}"#;
        assert_eq!(
            json_string_field(json, "build_directory").as_deref(),
            Some(r"D:\Users\x\build")
        );
    }

    #[test]
    fn an_absent_field_is_none_rather_than_an_empty_path() {
        assert_eq!(json_string_field(r#"{"a":1}"#, "build_directory"), None);
    }
}
