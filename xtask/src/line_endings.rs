//! Fails when a text file in the working tree has CRLF line endings.
//!
//! `.gitattributes` makes every text file LF in the index and in every checkout, so a CRLF file in
//! the working tree was written by a tool after checkout, most often Python on Windows, which
//! translates `\n` to `\r\n` in text mode. Git normalises it again on commit, so the history never
//! shows it, but the file on disk is what runs and what the other checks read: bash takes the `\r`
//! as part of a command, and a byte comparison fails on text that is identical.
//!
//! A file whose attributes say `-text` is the format's own business (RFC 5322 mail, vCards) and is
//! left alone.
//!
//! `cargo xtask fix-line-endings` rewrites the files this check names. `git checkout` cannot: it
//! skips a file whose content, once normalised, already matches the index, which a CRLF copy of an
//! LF file does.

use std::{path::Path, process::Command};

/// Runs the check. `Ok(true)` means no text file in the working tree has a CRLF.
///
/// # Errors
///
/// Fails when git cannot be run or reports an error.
pub(crate) fn run(root: &Path) -> Result<bool, String> {
    let offenders = offenders(root)?;
    if offenders.is_empty() {
        println!("OK: every text file in the working tree is LF.");
        return Ok(true);
    }
    for path in &offenders {
        eprintln!("ERROR: {path} has CRLF line endings.");
    }
    eprintln!(
        "\nA tool rewrote these after checkout, or the checkout predates the LF rule in \
         .gitattributes. `cargo xtask fix-line-endings` makes them LF and changes nothing else. \
         Python on Windows writes CRLF unless it opens the file with newline=\"\\n\"."
    );
    Ok(false)
}

/// Rewrites every file [`run`] would name with LF line endings, leaving the rest of its bytes as
/// they are, so an edit in progress survives.
///
/// # Errors
///
/// Fails when git cannot be run, or a file cannot be read or written.
pub(crate) fn fix(root: &Path) -> Result<usize, String> {
    let offenders = offenders(root)?;
    for path in &offenders {
        let file = root.join(path);
        let bytes = std::fs::read(&file).map_err(|e| format!("could not read {path}: {e}"))?;
        std::fs::write(&file, without_cr_before_lf(&bytes))
            .map_err(|e| format!("could not write {path}: {e}"))?;
        println!("LF: {path}");
    }
    Ok(offenders.len())
}

/// `bytes` with every CR that ends a line removed. A lone CR is content, not a line ending.
fn without_cr_before_lf(bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(bytes.len());
    for (i, &b) in bytes.iter().enumerate() {
        if b != b'\r' || bytes.get(i + 1) != Some(&b'\n') {
            out.push(b);
        }
    }
    out
}

/// Every text file, tracked or not, with a CRLF in the working tree.
fn offenders(root: &Path) -> Result<Vec<String>, String> {
    let output = Command::new("git")
        .args([
            "ls-files",
            "-z",
            "--eol",
            "--cached",
            "--others",
            "--exclude-standard",
        ])
        .current_dir(root)
        .output()
        .map_err(|e| format!("could not run git ls-files: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "git ls-files failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    Ok(String::from_utf8_lossy(&output.stdout)
        .split('\0')
        .filter_map(offending_path)
        .map(str::to_owned)
        .collect())
}

/// The path in one `git ls-files --eol` entry when that file is text with a CRLF on disk.
///
/// An entry reads `i/<index> w/<worktree> attr/<attributes>\t<path>`. Only the working tree is
/// read: it is what runs and what the other checks compare, and `.gitattributes` normalises what a
/// commit stores, so the index side is git's to keep, not this check's.
fn offending_path(entry: &str) -> Option<&str> {
    let (info, path) = entry.split_once('\t')?;
    let mut fields = info.split_whitespace();
    fields.next()?.strip_prefix("i/")?;
    let worktree = fields.next()?.strip_prefix("w/")?;
    let attributes = info.split_once("attr/").map_or("", |(_, a)| a);
    let binary_by_choice = attributes.split_whitespace().any(|a| a == "-text");
    (matches!(worktree, "crlf" | "mixed") && !binary_by_choice).then_some(path)
}

#[cfg(test)]
mod tests {
    use super::{fix, offending_path, run, without_cr_before_lf};
    use crate::fixture::Repo;

    #[test]
    fn the_fix_takes_out_line_ending_crs_only() {
        assert_eq!(without_cr_before_lf(b"a\r\nb\rc\n\r\n"), b"a\nb\rc\n\n");
    }

    #[test]
    fn the_fix_makes_the_check_pass_and_keeps_the_text() {
        let repo = Repo::new(&[("script.sh", "#!/usr/bin/env bash\r\necho ok\r\n")]);
        assert_eq!(run(repo.root()), Ok(false));
        assert_eq!(fix(repo.root()), Ok(1));
        assert_eq!(run(repo.root()), Ok(true));
        assert_eq!(
            std::fs::read_to_string(repo.root().join("script.sh")).unwrap(),
            "#!/usr/bin/env bash\necho ok\n"
        );
    }

    #[test]
    fn reads_the_listing_git_prints() {
        assert_eq!(
            offending_path("i/lf    w/crlf  attr/text=auto eol=lf \tLICENSE"),
            Some("LICENSE")
        );
        assert_eq!(
            offending_path("i/      w/mixed attr/text=auto eol=lf \tnew file.sh"),
            Some("new file.sh")
        );
        assert_eq!(
            offending_path("i/lf    w/lf    attr/text=auto eol=lf \tCargo.toml"),
            None
        );
        assert_eq!(
            offending_path("i/crlf  w/crlf  attr/-text            \tgradlew.bat"),
            None
        );
        assert_eq!(offending_path(""), None);
    }

    #[test]
    fn an_lf_tree_passes() {
        let repo = Repo::new(&[("script.sh", "#!/usr/bin/env bash\necho ok\n")]);
        assert_eq!(run(repo.root()), Ok(true));
    }

    #[test]
    fn a_crlf_file_fails_whether_or_not_it_is_tracked() {
        let tracked = Repo::new(&[("script.sh", "#!/usr/bin/env bash\r\necho ok\r\n")]);
        assert_eq!(run(tracked.root()), Ok(false));

        let untracked = Repo::new(&[]);
        untracked.write("notes.txt", "one\r\ntwo\r\n");
        assert_eq!(run(untracked.root()), Ok(false));
    }

    #[test]
    fn a_format_marked_binary_keeps_its_crlf() {
        let repo = Repo::new(&[
            (".gitattributes", "*.eml -text\n"),
            ("message.eml", "Subject: hi\r\n\r\nbody\r\n"),
        ]);
        assert_eq!(run(repo.root()), Ok(true));
    }
}
