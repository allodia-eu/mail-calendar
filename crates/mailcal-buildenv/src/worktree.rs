//! A linked git worktree made without its gitignored `.env`.
//!
//! `git worktree add` copies no ignored file, so a worktree made that way builds without the
//! registrations its main checkout carries, and the app it builds drops Google, Microsoft and
//! Allodia sign-in while looking like a regression in whatever the worktree is for. A worktree
//! Claude Code makes copies the file (`.worktreeinclude`). A build without registrations is
//! legitimate, so nothing here fails one: it only finds the case worth a warning.

use std::{
    fs,
    path::{Path, PathBuf},
};

/// The `.env` of the checkout `root` was made from, when `root` is a linked git worktree with no
/// `.env` of its own and that checkout has one. `None` for a main checkout, a worktree that has its
/// own file, and a main checkout without one, which is a build without registrations on purpose.
#[must_use]
pub fn main_checkout_env(root: &Path) -> Option<PathBuf> {
    if root.join(".env").exists() {
        return None;
    }
    // A linked worktree's `.git` is a file naming its private git directory; a main checkout's is
    // a directory.
    let pointer = fs::read_to_string(root.join(".git")).ok()?;
    let private = root.join(pointer.trim().strip_prefix("gitdir:")?.trim());
    // The private directory names the shared one, relative to itself. Resolved before its parent
    // is taken: the parent of `worktrees/task/../..` is lexically `worktrees/task/..`.
    let common = private.join(fs::read_to_string(private.join("commondir")).ok()?.trim());
    let candidate = fs::canonicalize(common).ok()?.parent()?.join(".env");
    candidate.is_file().then_some(candidate)
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use super::main_checkout_env;

    /// A main checkout with `.git/` and a linked worktree under it, laid out as git lays them out,
    /// in a directory of the test's own.
    fn layout(test: &str, main_env: bool, worktree_env: bool) -> (PathBuf, PathBuf) {
        let base =
            std::env::temp_dir().join(format!("mailcal-buildenv-{}-{test}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let main = base.join("main");
        let worktree = main.join(".claude/worktrees/task");
        let private = main.join(".git/worktrees/task");
        fs::create_dir_all(&private).unwrap();
        fs::create_dir_all(&worktree).unwrap();
        fs::write(private.join("commondir"), "../..\n").unwrap();
        fs::write(
            worktree.join(".git"),
            format!("gitdir: {}\n", private.display()),
        )
        .unwrap();
        if main_env {
            fs::write(main.join(".env"), "MAILCAL_MS_CLIENT_ID=x\n").unwrap();
        }
        if worktree_env {
            fs::write(worktree.join(".env"), "MAILCAL_MS_CLIENT_ID=x\n").unwrap();
        }
        (main, worktree)
    }

    #[test]
    fn a_worktree_without_the_file_its_main_checkout_has_is_found() {
        let (main, worktree) = layout("found", true, false);
        let found = main_checkout_env(&worktree).expect("the main checkout's .env is named");
        assert_eq!(
            fs::canonicalize(found).unwrap(),
            fs::canonicalize(main.join(".env")).unwrap()
        );
    }

    #[test]
    fn a_worktree_with_its_own_file_is_left_alone() {
        let (_, worktree) = layout("own", true, true);
        assert_eq!(main_checkout_env(&worktree), None);
    }

    #[test]
    fn a_main_checkout_without_registrations_is_a_choice_not_a_mistake() {
        let (_, worktree) = layout("none", false, false);
        assert_eq!(
            main_checkout_env(&worktree),
            None,
            "nothing to copy, so nothing to say"
        );
    }

    #[test]
    fn a_main_checkout_is_never_asked_about_another() {
        let (main, _) = layout("main", false, false);
        fs::create_dir_all(main.join(".git")).ok();
        assert_eq!(
            main_checkout_env(&main),
            None,
            "its .git is a directory, not a pointer"
        );
    }

    #[test]
    fn a_relative_pointer_resolves_from_the_worktree() {
        let (main, worktree) = layout("relative", true, false);
        fs::write(
            worktree.join(".git"),
            "gitdir: ../../../.git/worktrees/task\n",
        )
        .unwrap();
        let found = main_checkout_env(&worktree)
            .expect("git writes a relative pointer under some settings");
        assert_eq!(
            fs::canonicalize(found).unwrap(),
            fs::canonicalize(main.join(".env")).unwrap()
        );
    }
}
