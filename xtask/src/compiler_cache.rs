//! Whether cargo builds through kache, said once at the end of a gate run.
//!
//! A note, never a failure: kache only shares compile work between worktrees, and a build without
//! it is the same build, slower (docs/debugging.md, "Share the compile work with kache").

use std::{
    path::{Path, PathBuf},
    process::Command,
};

/// What to tell the developer, or `None` when cargo's compiler wrapper is a kache that answers.
pub(crate) fn note(root: &Path) -> Option<String> {
    const COST: &str = "so a new worktree's first build compiles every dependency itself, about \
                        two and a half minutes more for mailcal-bindings alone. Setting it up: \
                        docs/debugging.md, \"Share the compile work with kache\".";
    let configs: Vec<String> = config_files(root)
        .iter()
        .filter_map(|file| std::fs::read_to_string(file).ok())
        .collect();
    match wrapper(|name| std::env::var(name).ok(), &configs) {
        None => Some(format!("cargo is not building through kache, {COST}")),
        Some(wrapper) if !named_kache(&wrapper) => Some(format!(
            "cargo's compiler wrapper is `{wrapper}`, not kache, {COST}"
        )),
        Some(wrapper) if !answers_as_kache(&wrapper) => Some(format!(
            "cargo's compiler wrapper `{wrapper}` does not answer `--version` as kache, {COST}"
        )),
        Some(_) => None,
    }
}

/// The wrapper cargo would use, by cargo's own precedence: `RUSTC_WRAPPER`, then
/// `CARGO_BUILD_RUSTC_WRAPPER`, then `build.rustc-wrapper` in the nearest config file that sets
/// it. An empty value is cargo's way of saying *no wrapper*, wherever it is set.
fn wrapper(env: impl Fn(&str) -> Option<String>, configs: &[String]) -> Option<String> {
    let chosen = env("RUSTC_WRAPPER")
        .or_else(|| env("CARGO_BUILD_RUSTC_WRAPPER"))
        .or_else(|| configs.iter().find_map(|text| configured(text)))?;
    let chosen = chosen.trim().to_owned();
    (!chosen.is_empty()).then_some(chosen)
}

/// `build.rustc-wrapper` in one config file, as `[build]` `rustc-wrapper = "…"` or as the dotted
/// key. Not a TOML parser: a string value on one line is the only shape cargo's own docs show, and
/// `kache init` writes.
fn configured(text: &str) -> Option<String> {
    let mut table = String::new();
    for line in text.lines() {
        let line = line.trim();
        if let Some(header) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            header.trim().clone_into(&mut table);
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        let is_wrapper = (table == "build" && key == "rustc-wrapper")
            || (table.is_empty() && key == "build.rustc-wrapper");
        if is_wrapper {
            return Some(unquote(value.trim()));
        }
    }
    None
}

/// A TOML basic or literal string's contents; a basic string's escaped backslashes undone, which
/// is how a Windows path is written in one.
fn unquote(value: &str) -> String {
    if let Some(inner) = value.strip_prefix('\'').and_then(|v| v.split_once('\'')) {
        return inner.0.to_owned();
    }
    if let Some(inner) = value.strip_prefix('"').and_then(|v| v.split_once('"')) {
        return inner.0.replace("\\\\", "\\");
    }
    value.to_owned()
}

/// Whether `wrapper` names a program called kache, by path or by bare name.
fn named_kache(wrapper: &str) -> bool {
    Path::new(wrapper)
        .file_stem()
        .is_some_and(|stem| stem.eq_ignore_ascii_case("kache"))
}

/// Whether the program says it is kache: a name is not evidence (AGENTS.md, "Identify a tool by
/// what it says").
fn answers_as_kache(wrapper: &str) -> bool {
    Command::new(wrapper)
        .arg("--version")
        .output()
        .is_ok_and(|out| out.status.success() && out.stdout.starts_with(b"kache "))
}

/// Every config file cargo would read for a build in `root`, nearest first: each `.cargo/` from
/// the checkout up to the filesystem root, then cargo's home.
fn config_files(root: &Path) -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = root.ancestors().map(|dir| dir.join(".cargo")).collect();
    if let Some(home) = cargo_home() {
        dirs.push(home);
    }
    dirs.into_iter()
        .flat_map(|dir| [dir.join("config.toml"), dir.join("config")])
        .collect()
}

fn cargo_home() -> Option<PathBuf> {
    if let Some(home) = std::env::var_os("CARGO_HOME") {
        return Some(PathBuf::from(home));
    }
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(|home| PathBuf::from(home).join(".cargo"))
}

#[cfg(test)]
mod tests {
    use super::{configured, named_kache, wrapper};

    fn env<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
        move |name| {
            pairs
                .iter()
                .find(|(key, _)| *key == name)
                .map(|(_, value)| (*value).to_owned())
        }
    }

    #[test]
    fn the_environment_answers_before_any_config_file() {
        let configs = ["[build]\nrustc-wrapper = \"sccache\"\n".to_owned()];
        assert_eq!(
            wrapper(env(&[("RUSTC_WRAPPER", "kache")]), &configs).as_deref(),
            Some("kache")
        );
        assert_eq!(
            wrapper(env(&[("CARGO_BUILD_RUSTC_WRAPPER", "kache")]), &configs).as_deref(),
            Some("kache")
        );
        assert_eq!(wrapper(env(&[]), &configs).as_deref(), Some("sccache"));
    }

    #[test]
    fn an_empty_wrapper_is_no_wrapper_even_over_a_config_file() {
        let configs = ["[build]\nrustc-wrapper = \"kache\"\n".to_owned()];
        assert_eq!(wrapper(env(&[("RUSTC_WRAPPER", "")]), &configs), None);
    }

    #[test]
    fn the_nearest_config_file_that_sets_it_wins() {
        let configs = [
            "[alias]\nxtask = 'run'\n".to_owned(),
            "build.rustc-wrapper = 'C:\\tools\\kache.exe'\n".to_owned(),
            "[build]\nrustc-wrapper = \"sccache\"\n".to_owned(),
        ];
        assert_eq!(
            wrapper(env(&[]), &configs).as_deref(),
            Some("C:\\tools\\kache.exe")
        );
    }

    #[test]
    fn a_key_of_the_same_name_in_another_table_is_not_the_wrapper() {
        assert_eq!(
            configured("[target.x86_64-pc-windows-msvc]\nrustc-wrapper = \"kache\"\n"),
            None
        );
        assert_eq!(
            configured("[build]\nrustflags = \"-C x\"\n[env]\nrustc-wrapper = \"kache\"\n"),
            None
        );
    }

    #[test]
    fn a_windows_path_in_a_basic_string_loses_its_escapes() {
        assert_eq!(
            configured("[build]\nrustc-wrapper = \"C:\\\\Users\\\\me\\\\.cargo\\\\bin\\\\kache.exe\" # by kache init\n")
                .as_deref(),
            Some("C:\\Users\\me\\.cargo\\bin\\kache.exe")
        );
    }

    #[test]
    fn kache_is_recognised_by_its_name_alone_whatever_the_path() {
        assert!(named_kache("kache"));
        assert!(named_kache("C:\\Users\\me\\.cargo\\bin\\kache.exe"));
        assert!(named_kache("/home/me/.cargo/bin/kache"));
        assert!(!named_kache("sccache"));
        assert!(!named_kache("/usr/bin/kachex"));
    }
}
