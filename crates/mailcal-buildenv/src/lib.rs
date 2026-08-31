//! Shared build-script access to the values Allodia injects at compile time.
//!
//! Two build scripts inject values a client cannot supply and a shipped binary must carry: the
//! OAuth client registrations (`mailcal-oauth`) and the analytics relay endpoint
//! (`mailcal-bindings`). Both resolve a name the same way, both have to survive the same five build
//! front doors, and both must refuse to produce an artifact **we ship** that quietly lost one. The
//! mechanism lives here so the two cannot drift; each build script keeps its own policy, which
//! names it wants, and what to say when they are missing.
//!
//! Why a crate rather than a second copy of the same ninety lines: this parser decides whether a
//! shipped binary carries its credentials, and a build script's own `#[cfg(test)]` never runs, so
//! until now nothing tested it. A library's tests run under `cargo test --workspace`.
//!
//! # Resolution order
//!
//! **The environment always wins**, then the repo's gitignored `.env`. That is the order
//! `option_env!` itself will see: a value already in the environment reaches the compiler without
//! help, so only a file-supplied value is worth emitting. CI hands values in as secrets; a
//! developer overrides one for a single build without editing the file.
//!
//! A name that is **set but blank counts as unset**, here and at every reader: a CI run without
//! access to the secrets sets the empty string rather than leaving the name unbound.

use std::{env, fs, path::PathBuf};

/// Set this (to anything but `0`, `false`, `no` or empty) and a missing injected value fails the
/// build instead of producing an artifact without it.
///
/// Off by default: a from-source build without Allodia's registrations or relay is supported and
/// must stay that way; it simply drops the sign-in routes and sends no analytics. Only the paths
/// that produce something we ship set it.
pub const REQUIRE_VAR: &str = "MAILCAL_REQUIRE_INJECTED_CONFIG";

/// The name [`REQUIRE_VAR`] used to have, still honoured so a packaging path or a CI job that has
/// not been updated keeps its guarantee rather than silently losing it. Either name turns the
/// requirement on.
pub const LEGACY_REQUIRE_VAR: &str = "MAILCAL_REQUIRE_OAUTH_CREDENTIALS";

/// The repo's `.env`, read once, plus the cargo directives that keep a build script honest about
/// it.
#[derive(Debug)]
pub struct Injected {
    contents: String,
}

impl Injected {
    /// Reads the repo's `.env` and emits the freshness directives every caller needs.
    ///
    /// The `rerun-if-changed` for the file is emitted whether or not it exists: a path that does
    /// not exist re-runs the script on every build, which is how creating `.env` for the first time
    /// gets noticed. Naming any rerun condition replaces cargo's default of watching the whole
    /// package, so the caller's own `build.rs` is registered here too.
    #[must_use]
    pub fn load() -> Self {
        println!("cargo::rerun-if-changed=build.rs");
        println!("cargo::rerun-if-env-changed={REQUIRE_VAR}");
        println!("cargo::rerun-if-env-changed={LEGACY_REQUIRE_VAR}");

        let path = repo_root().join(".env");
        println!("cargo::rerun-if-changed={}", path.display());
        Self {
            contents: fs::read_to_string(&path).unwrap_or_default(),
        }
    }

    /// Makes `vars` visible to `option_env!` in the crate being built, and rebuilds that crate when
    /// any of them changes.
    ///
    /// `option_env!` is resolved by the compiler and cargo does not know it happened, so without
    /// the `rerun-if-env-changed` here, filling in a name that was empty leaves the cached build in
    /// place: a stale binary that reads as a broken feature rather than a stale one.
    pub fn export(&self, vars: &[&str]) {
        for var in vars {
            println!("cargo::rerun-if-env-changed={var}");
            // Only a value the file supplies and the environment does not is worth emitting:
            // `option_env!` already sees the environment, and it outranks the file.
            if present(env::var(var).ok()).is_none()
                && let Some(value) = self.file_value(var)
            {
                println!("cargo::rustc-env={var}={value}");
            }
        }
    }

    /// A name's value as `option_env!` will see it: environment first, `.env` second.
    #[must_use]
    pub fn value_of(&self, var: &str) -> Option<String> {
        present(env::var(var).ok()).or_else(|| self.file_value(var))
    }

    /// Which of `vars` no source supplies. Empty means the build has everything it asked for.
    #[must_use]
    pub fn missing<'a>(&self, vars: &[&'a str]) -> Vec<&'a str> {
        vars.iter()
            .filter(|var| self.value_of(var).is_none())
            .copied()
            .collect()
    }

    /// Whether this build must refuse to be produced without its injected values; [`REQUIRE_VAR`]
    /// or its legacy spelling, from the environment or the file.
    ///
    /// The file is read as well as the environment because the Flatpak build runs cargo inside a
    /// sandbox that forwards no host environment, so the file is the only way in.
    #[must_use]
    pub fn is_required(&self) -> bool {
        truthy(self.value_of(REQUIRE_VAR).as_deref())
            || truthy(self.value_of(LEGACY_REQUIRE_VAR).as_deref())
    }

    /// `KEY=value` for `key`, if the file names it.
    ///
    /// Deliberately not a shell, and the same shape as `scripts/dev/envfile.py`: `export ` and
    /// surrounding quotes are tolerated, comments and blanks skipped, and nothing is interpolated
    /// or executed. A credentials file is a few lines, and a parser that can run something is a
    /// parser that can be made to run something.
    fn file_value(&self, key: &str) -> Option<String> {
        self.contents.lines().find_map(|line| {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                return None;
            }
            let line = line.strip_prefix("export ").unwrap_or(line);
            let (name, value) = line.split_once('=')?;
            if name.trim() != key {
                return None;
            }
            present(Some(unquote(value.trim()).to_owned()))
        })
    }
}

/// The repository root, from the calling crate's own directory rather than the working directory;
/// Gradle, `xcodebuild` and the packaging scripts all invoke cargo from somewhere different.
///
/// # Panics
///
/// If cargo did not set `CARGO_MANIFEST_DIR`, which means this is not running as a build script.
#[must_use]
pub fn repo_root() -> PathBuf {
    PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR"))
        .join("../..")
}

/// A value with one layer of matching quotes taken off.
fn unquote(value: &str) -> &str {
    for quote in ['"', '\''] {
        if let Some(inner) = value
            .strip_prefix(quote)
            .and_then(|v| v.strip_suffix(quote))
        {
            return inner;
        }
    }
    value
}

/// A value that says something, trimmed. Unset and blank are the same answer, here and at every
/// reader.
fn present(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

/// Whether a flag value means yes. `0`, `false` and `no` mean no, so a script may pass a computed
/// value (`MAILCAL_REQUIRE_INJECTED_CONFIG=$IS_RELEASE`) without having to unset the name instead.
fn truthy(value: Option<&str>) -> bool {
    !matches!(
        value.map_or("", str::trim).to_ascii_lowercase().as_str(),
        "" | "0" | "false" | "no"
    )
}

#[cfg(test)]
mod tests {
    use super::{Injected, present, truthy, unquote};

    /// Builds an `Injected` over literal file contents, bypassing the repo lookup: the parser is
    /// what these tests are about, not where the file lives.
    fn file(contents: &str) -> Injected {
        Injected {
            contents: contents.to_owned(),
        }
    }

    #[test]
    fn reads_a_plain_assignment() {
        assert_eq!(
            file("A=one").file_value("A"),
            Some("one".to_owned()),
            "the simplest line the file can hold"
        );
    }

    #[test]
    fn tolerates_export_and_quotes_and_surrounding_space() {
        let env = file("export A=\"one\"\n  B = 'two' \n");
        assert_eq!(env.file_value("A"), Some("one".to_owned()));
        assert_eq!(env.file_value("B"), Some("two".to_owned()));
    }

    #[test]
    fn skips_comments_and_blank_lines() {
        let env = file("# A=commented\n\nA=real\n");
        assert_eq!(
            env.file_value("A"),
            Some("real".to_owned()),
            "a commented-out name must not resolve"
        );
    }

    #[test]
    fn does_not_match_a_name_by_prefix() {
        let env = file("ALLODIA_TELEMETRY_URL_OLD=stale\n");
        assert_eq!(
            env.file_value("ALLODIA_TELEMETRY_URL"),
            None,
            "a longer name that starts with ours is a different variable"
        );
    }

    #[test]
    fn a_blank_value_reads_as_absent() {
        assert_eq!(file("A=\nB=\"\"\n").file_value("A"), None);
        assert_eq!(file("A=\nB=\"\"\n").file_value("B"), None);
        assert_eq!(present(Some("   ".to_owned())), None);
    }

    #[test]
    fn a_value_may_contain_the_separator() {
        assert_eq!(
            file("URL=https://telemetry.allodia.eu/v1?a=b").file_value("URL"),
            Some("https://telemetry.allodia.eu/v1?a=b".to_owned()),
            "only the first `=` separates the name from the value"
        );
    }

    #[test]
    fn missing_names_only_the_ones_no_source_supplies() {
        let env = file("A=one\n");
        assert_eq!(env.missing(&["A"]), Vec::<&str>::new());
        assert_eq!(env.missing(&["A", "B"]), vec!["B"]);
    }

    #[test]
    fn the_requirement_is_off_unless_asked_for() {
        assert!(!file("").is_required(), "a from-source build is supported");
        assert!(file("MAILCAL_REQUIRE_INJECTED_CONFIG=1").is_required());
    }

    #[test]
    fn the_legacy_flag_still_turns_the_requirement_on() {
        assert!(
            file("MAILCAL_REQUIRE_OAUTH_CREDENTIALS=1").is_required(),
            "a packaging path or CI job still on the old name keeps its guarantee"
        );
    }

    #[test]
    fn a_falsey_flag_is_not_a_requirement() {
        for value in ["0", "false", "no", "", "  "] {
            assert!(
                !truthy(Some(value)),
                "{value:?} must read as off so a script can pass a computed value"
            );
        }
        assert!(truthy(Some("1")));
    }

    #[test]
    fn unquote_takes_off_one_matching_pair_only() {
        assert_eq!(unquote("\"one\""), "one");
        assert_eq!(unquote("'one'"), "one");
        assert_eq!(
            unquote("\"one"),
            "\"one",
            "an unmatched quote is part of the value"
        );
        assert_eq!(
            unquote("\"'one'\""),
            "'one'",
            "only the outer pair comes off"
        );
    }
}
