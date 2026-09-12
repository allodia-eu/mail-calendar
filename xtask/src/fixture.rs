//! A throwaway git repository, for tests that need a check to run over a whole tree.
//!
//! The checks that search take a repository root and shell out to git, so the only honest way to
//! test one is to give it a real repository with real files in it. Each fixture is created under
//! the system temporary directory and removed when the test ends.

use std::{
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicUsize, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

/// Distinguishes fixtures created within the same nanosecond, since tests run in parallel.
static COUNTER: AtomicUsize = AtomicUsize::new(0);

/// A temporary git repository, deleted on drop.
#[derive(Debug)]
pub(crate) struct Repo {
    root: PathBuf,
}

impl Repo {
    /// Creates a repository containing `files`, each given as a repo-relative path and its content.
    ///
    /// Two directories are always present. `git grep` fails outright on a pathspec that matches no
    /// file, and several checks name `crates` and `clients` explicitly, so a fixture without them
    /// would report an error rather than the verdict under test.
    pub(crate) fn new(files: &[(&str, &str)]) -> Self {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!("xtask-fixture-{stamp}-{unique}"));
        std::fs::create_dir_all(&root).expect("could not create the fixture directory");

        let repo = Self { root };
        repo.git(&["init", "--quiet"]);
        repo.write("crates/.keep", "");
        repo.write("clients/.keep", "");
        for (name, content) in files {
            repo.write(name, content);
        }
        // Tracked, because a check that lists files reads the index. Untracked files are covered
        // too, since the searches ask for them, so this only widens what the fixture can express.
        repo.git(&["add", "-A"]);
        repo
    }

    /// The repository root, to hand to a check.
    pub(crate) fn root(&self) -> &Path {
        &self.root
    }

    /// Writes one file, creating its parent directories.
    pub(crate) fn write(&self, name: &str, content: &str) {
        let path = self.root.join(name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("could not create a fixture subdirectory");
        }
        std::fs::write(path, content).expect("could not write a fixture file");
    }

    /// Runs git inside the fixture.
    fn git(&self, args: &[&str]) {
        let status = Command::new("git")
            .args(args)
            .current_dir(&self.root)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .expect("could not run git in the fixture");
        assert!(status.success(), "git {args:?} failed in the fixture");
    }
}

impl Drop for Repo {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}
