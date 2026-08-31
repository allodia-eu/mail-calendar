//! Vendored binshim for NordSecurity's `uniffi-bindgen-cs`: re-exposes its `main()` as a
//! `cargo run` target so the C# bindings are generated with a `Cargo.lock`-pinned generator
//! instead of a separately `cargo install`ed binary. The Windows client build scripts call it as
//! `cargo run -p mailcal-bindgen-cs -- --library <cdylib> --out-dir <dir>` (same args, same code).
//!
//! One behaviour is added on top: the generator rewrites its output file on every run, and a
//! byte-identical rewrite still moves the file's mtime, which is all MSBuild compares, so the
//! WinUI app recompiled on every build even when the FFI had not moved. The mtime of a file
//! generation left identical is put back afterwards. It has to happen after the fact rather than
//! by generating somewhere else first: upstream's `main()` parses the arguments itself and keeps
//! both its argument type and its `BindingGenerator` private, so there is no seam to redirect.

use std::{
    fs::{File, FileTimes},
    io,
    path::{Path, PathBuf},
    time::SystemTime,
};

fn main() {
    let before = out_dir_arg(std::env::args()).map_or_else(Vec::new, |dir| snapshot(&dir));

    if let Err(err) = uniffi_bindgen_cs::main() {
        eprintln!("uniffi-bindgen-cs: {err:?}");
        std::process::exit(1);
    }

    for file in &before {
        match restore_if_unchanged(file) {
            Ok(true) => println!("unchanged, timestamp kept: {}", file.path.display()),
            Ok(false) => {}
            // Losing the mtime costs a rebuild, never correctness, so say so and carry on.
            Err(err) => eprintln!(
                "uniffi-bindgen-cs: could not preserve the timestamp of {}: {err}",
                file.path.display()
            ),
        }
    }
}

/// A generated file as it stood before generation ran.
struct Snapshot {
    path: PathBuf,
    bytes: Vec<u8>,
    modified: SystemTime,
}

/// The output directory the generator was pointed at, in any spelling clap accepts for
/// `--out-dir` / `-o`. `None` when the arguments do not name one, which costs a rebuild rather
/// than a wrong answer.
fn out_dir_arg(args: impl Iterator<Item = String>) -> Option<PathBuf> {
    let mut args = args.skip(1);
    while let Some(arg) = args.next() {
        for flag in ["--out-dir", "-o"] {
            if arg == flag {
                return args.next().map(PathBuf::from);
            }
            if let Some(value) = arg.strip_prefix(&format!("{flag}=")) {
                return Some(PathBuf::from(value));
            }
        }
    }
    None
}

/// Reads every file directly in `dir`. A directory that does not exist yet snapshots as empty,
/// so a first run has nothing to preserve.
fn snapshot(dir: &Path) -> Vec<Snapshot> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            let meta = entry.metadata().ok()?;
            if !meta.is_file() {
                return None;
            }
            Some(Snapshot {
                bytes: std::fs::read(&path).ok()?,
                modified: meta.modified().ok()?,
                path,
            })
        })
        .collect()
}

/// Puts `file`'s recorded mtime back when generation rewrote it with the same bytes. Reports
/// whether it did.
///
/// # Errors
///
/// Returns the underlying error when the file cannot be read or its times cannot be set.
fn restore_if_unchanged(file: &Snapshot) -> io::Result<bool> {
    if std::fs::read(&file.path)? != file.bytes {
        return Ok(false);
    }
    // Opening for write does not truncate without `truncate(true)`, nothing here changes a byte.
    File::options()
        .write(true)
        .open(&file.path)?
        .set_times(FileTimes::new().set_modified(file.modified))?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use std::{path::PathBuf, time::Duration};

    use super::{Snapshot, out_dir_arg, restore_if_unchanged, snapshot};

    fn args(rest: &[&str]) -> impl Iterator<Item = String> {
        let mut all = vec!["uniffi-bindgen-cs".to_string()];
        all.extend(rest.iter().map(|s| (*s).to_string()));
        all.into_iter()
    }

    #[test]
    fn reads_the_out_dir_in_every_spelling_clap_accepts() {
        let lib = ["--library", "libmailcal_bindings.so"];
        for spelling in [
            vec!["--out-dir", "Generated"],
            vec!["--out-dir=Generated"],
            vec!["-o", "Generated"],
            vec!["-o=Generated"],
        ] {
            let mut argv = lib.to_vec();
            argv.extend(spelling.iter().copied());
            assert_eq!(
                out_dir_arg(args(&argv)),
                Some(PathBuf::from("Generated")),
                "{spelling:?} names the output directory"
            );
        }
    }

    #[test]
    fn no_out_dir_reads_as_nothing_to_preserve() {
        assert_eq!(
            out_dir_arg(args(&["--library", "libmailcal_bindings.so"])),
            None
        );
    }

    #[test]
    fn an_identical_rewrite_keeps_its_timestamp_and_a_real_one_does_not() {
        // MSBuild compares mtimes, so a generator that touches an unchanged file rebuilds the
        // whole WinUI app for nothing.
        let dir = std::env::temp_dir().join("mailcal-bindgen-cs-timestamp-test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("the test directory is creatable");
        let path = dir.join("mailcal_bindings.cs");
        std::fs::write(&path, b"// bindings").expect("the fixture is writable");

        let before = snapshot(&dir);
        assert_eq!(before.len(), 1, "the snapshot covers the generated file");
        let stamp = before[0].modified;

        // What the generator does: rewrite the same bytes, moving the mtime.
        std::thread::sleep(Duration::from_millis(20));
        std::fs::write(&path, b"// bindings").expect("the rewrite succeeds");
        let touched = std::fs::metadata(&path)
            .and_then(|m| m.modified())
            .expect("the rewritten file has a modification time");
        assert_ne!(touched, stamp, "a plain rewrite moves the mtime");

        assert!(
            restore_if_unchanged(&before[0]).expect("the timestamp is restorable"),
            "identical bytes are reported as unchanged"
        );
        assert_eq!(
            std::fs::metadata(&path).and_then(|m| m.modified()).ok(),
            Some(stamp),
            "the mtime is put back"
        );
        assert_eq!(
            std::fs::read(&path).expect("the file still reads"),
            b"// bindings",
            "restoring a timestamp must not disturb the contents"
        );

        // A real FFI change must still look new.
        std::fs::write(&path, b"// different bindings").expect("the edit succeeds");
        assert!(
            !restore_if_unchanged(&before[0]).expect("the check succeeds"),
            "changed bytes keep their new mtime"
        );

        // A file the generator deleted is not resurrected.
        std::fs::remove_file(&path).expect("the fixture is removable");
        assert!(restore_if_unchanged(&before[0]).is_err());
        assert!(!path.exists());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_directory_that_does_not_exist_yet_snapshots_as_empty() {
        let dir = std::env::temp_dir().join("mailcal-bindgen-cs-absent-dir-test");
        let _ = std::fs::remove_dir_all(&dir);
        assert!(snapshot(&dir).is_empty());
    }

    #[test]
    fn a_subdirectory_is_not_snapshotted_as_a_file() {
        let dir = std::env::temp_dir().join("mailcal-bindgen-cs-subdir-test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("nested")).expect("the test tree is creatable");
        std::fs::write(dir.join("mailcal_bindings.cs"), b"// bindings").expect("writable");

        let taken: Vec<_> = snapshot(&dir)
            .iter()
            .map(|s: &Snapshot| s.path.clone())
            .collect();
        assert_eq!(taken, vec![dir.join("mailcal_bindings.cs")]);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
