//! What this build is called, and what the operating system knows it by.
//!
//! Two values (`docs/branding.md`), reaching the clients two different ways:
//!
//! - **The name.** The catalog writes `{app_name}` wherever the product names itself: the window
//!   title, the welcome heading, the attributions line, and it is substituted here, before
//!   validation, rather than left as a message parameter. The name is one string for all seven
//!   languages, so a per-locale copy could only drift, and a parameter would oblige every call site
//!   in four clients to pass a constant that never varies.
//! - **The application id**, emitted as a constant for the two clients whose platform cannot be
//!   asked at runtime: the Linux binary is not a bundle, and the Windows app runs unpackaged in the
//!   dev loop, where `Package.Current` throws. Android reads `BuildConfig.APPLICATION_ID` and Apple
//!   reads `Bundle.main`, which are truer answers still; they are what actually got installed.
//!
//! Both come from the same files every other build system reads, by the same rule. So does the
//! publisher's privacy policy, which the catalog writes as `{privacy_url}` for the same reason it
//! writes `{app_name}`: it is the publisher's address, one for every language.

use std::{collections::BTreeMap, path::Path};

use crate::model::Raw;

/// The placeholder the catalog uses for the product's own name.
const APP_NAME_PLACEHOLDER: &str = "{app_name}";

/// The variables that carry the identity, in the environment and in the brand files alike.
const APP_NAME_KEY: &str = "MAILCAL_APP_NAME";
const APP_ID_KEY: &str = "MAILCAL_APP_ID";
const MCP_SERVER_NAME_KEY: &str = "MAILCAL_MCP_SERVER_NAME";
const MCP_RELAY_NAME_KEY: &str = "MAILCAL_MCP_RELAY_NAME";

/// Every brand value the catalog writes as a placeholder, and the variable that fills it.
const PLACEHOLDERS: [(&str, &str); 2] = [
    (APP_NAME_PLACEHOLDER, APP_NAME_KEY),
    ("{privacy_url}", "MAILCAL_PRIVACY_URL"),
];

/// The brand files, in the order they answer: the publisher's when the checkout has it (under its
/// name, then under the older name it is still read by), then the neutral default every checkout
/// has.
const BRAND_FILES: [&str; 3] = ["brand.env", "allodia.env", "default.env"];

/// The identity this build carries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Brand {
    /// What the app calls itself, in every language.
    pub name: String,
    /// The reverse-DNS id the OS knows it by, which is also a URI scheme.
    pub id: String,
    /// The key an MCP client files the local server under, which is also the server's own
    /// `serverInfo.name`.
    pub mcp_server_name: String,
    /// The command the MCP relay is installed as, which an MCP client spawns.
    pub mcp_relay_name: String,
}

/// Resolves the identity: the environment, then the publisher's brand file when the checkout has
/// one, then the neutral default that every checkout has.
///
/// # Errors
///
/// Returns a message when nothing gives a value. `branding/default.env` is committed and carries
/// both, so that means the checkout is incomplete rather than unbranded: the unbranded case is a
/// *value*, not an absence.
pub(crate) fn resolve(root: &Path) -> Result<Brand, String> {
    Ok(Brand {
        name: value(
            root,
            APP_NAME_KEY,
            std::env::var(APP_NAME_KEY).ok().as_deref(),
        )?,
        id: value(root, APP_ID_KEY, std::env::var(APP_ID_KEY).ok().as_deref())?,
        mcp_server_name: resolve_value(root, MCP_SERVER_NAME_KEY)?,
        mcp_relay_name: resolve_value(root, MCP_RELAY_NAME_KEY)?,
    })
}

/// The product token the build names itself by in an HTTP `User-Agent` (RFC 9110 §10.1.5): the
/// name in lowercase, each run of anything but an ASCII letter or digit as one hyphen.
/// "Allodia Mail & Calendar" is `allodia-mail-calendar`; `None` when nothing is left.
pub(crate) fn product_token(name: &str) -> Option<String> {
    let token = name
        .to_ascii_lowercase()
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    (!token.is_empty()).then_some(token)
}

/// Every placeholder in [`PLACEHOLDERS`] with the value this build gives it.
///
/// # Errors
///
/// Returns a message when a placeholder's variable resolves nowhere, which `branding/default.env`
/// rules out in a complete checkout.
pub(crate) fn placeholders(root: &Path) -> Result<Vec<(&'static str, String)>, String> {
    PLACEHOLDERS
        .iter()
        .map(|&(placeholder, key)| Ok((placeholder, resolve_value(root, key)?)))
        .collect()
}

/// One brand value by the rule every reader uses, for a build script that injects it. Prints the
/// `cargo:rerun-if-*` lines that keep the build from holding on to an older answer.
///
/// # Errors
///
/// Returns a message when nothing gives the key a value.
pub(crate) fn resolve_value(root: &Path, key: &str) -> Result<String, String> {
    value(root, key, std::env::var(key).ok().as_deref())
}

/// Replaces each placeholder with its value in every message of every locale.
pub(crate) fn replace_placeholders(raw: &mut Raw, values: &[(&str, String)]) {
    for messages in raw.maps.values_mut() {
        for template in messages.values_mut() {
            for (placeholder, found) in values {
                if template.contains(placeholder) {
                    *template = template.replace(placeholder, found);
                }
            }
        }
    }
}

fn value(root: &Path, key: &str, from_environment: Option<&str>) -> Result<String, String> {
    // Everything that could answer, named before anything does. Harmless outside a build script:
    // the lines go to stdout, which only Cargo reads.
    for line in tracking(root, key) {
        println!("{line}");
    }
    if let Some(found) = from_environment.map(str::trim).filter(|v| !v.is_empty()) {
        return Ok(found.to_string());
    }
    let branding = root.join("branding");
    for file in BRAND_FILES {
        let Ok(text) = std::fs::read_to_string(branding.join(file)) else {
            continue;
        };
        if let Some(found) = parse(&text).remove(key) {
            return Ok(found);
        }
    }
    Err(format!(
        "no {key} in the environment or under {}: the neutral default belongs in \
         branding/default.env and is not optional there",
        branding.display()
    ))
}

/// Everything an identity can come from, named for Cargo so that a build does not keep the one it
/// was first given.
///
/// **The variable, because it answers first and the dev scripts export it**: `lib.sh` calls
/// `brand_load`, so a scripted build reads its identity from the environment and touches no brand
/// file at all. A build keyed on the files alone is therefore never invalidated by the identity
/// changing, and the catalog goes on carrying the name it was generated with.
///
/// **The directory, and not only the files in it**, because Cargo drops a `rerun-if-changed`
/// naming a path that does not exist, so a brand file *appearing* would rebuild nothing. Creating
/// it moves the directory's mtime, which Cargo does act on.
fn tracking(root: &Path, key: &str) -> Vec<String> {
    let branding = root.join("branding");
    let mut lines = vec![
        format!("cargo:rerun-if-env-changed={key}"),
        format!("cargo:rerun-if-changed={}", branding.display()),
    ];
    lines.extend(
        BRAND_FILES
            .iter()
            .map(|file| format!("cargo:rerun-if-changed={}", branding.join(file).display())),
    );
    lines
}

/// `KEY=value` lines, tolerating comments, blanks, `export ` and one pair of quotes. Deliberately
/// not a shell, and deliberately a second small copy rather than a shared dependency: the OAuth
/// build script reads a different file for a different reason, and joining them would make a
/// codegen crate a build input of the credential injection.
fn parse(text: &str) -> BTreeMap<String, String> {
    let mut values = BTreeMap::new();
    for line in text.lines() {
        let line = line.trim();
        let line = line.strip_prefix("export ").unwrap_or(line).trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let value = value.trim();
        let value = value
            .strip_circumfix('"', '"')
            .or_else(|| value.strip_circumfix('\'', '\''))
            .unwrap_or(value);
        if !value.is_empty() {
            values.insert(key.trim().to_string(), value.to_string());
        }
    }
    values
}

#[cfg(test)]
mod tests {
    use std::{
        collections::BTreeMap,
        path::{Path, PathBuf},
    };

    use super::{
        APP_ID_KEY, APP_NAME_KEY, APP_NAME_PLACEHOLDER, PLACEHOLDERS, parse, product_token,
        replace_placeholders, tracking, value,
    };
    use crate::model::Raw;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(name);
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("branding")).expect("a scratch checkout");
        dir
    }

    fn write(root: &Path, file: &str, contents: &str) {
        std::fs::write(root.join("branding").join(file), contents).expect("write a brand file");
    }

    fn raw_with(template: &str) -> Raw {
        let mut messages = BTreeMap::new();
        messages.insert("app_title".to_string(), template.to_string());
        messages.insert("nav_mail".to_string(), "Mail".to_string());
        let mut maps = BTreeMap::new();
        maps.insert("en".to_string(), messages.clone());
        maps.insert("nl".to_string(), messages);
        Raw {
            base: "en".to_string(),
            locales: vec!["en".to_string(), "nl".to_string()],
            maps,
        }
    }

    #[test]
    fn quotes_comments_and_export_are_all_tolerated() {
        let values = parse("# a note\n\nexport MAILCAL_APP_NAME=\"Two Words\"\nQ='x'\nBLANK=\n");

        assert_eq!(
            values.get("MAILCAL_APP_NAME").map(String::as_str),
            Some("Two Words")
        );
        assert_eq!(values.get("Q").map(String::as_str), Some("x"));
        assert_eq!(
            values.get("BLANK"),
            None,
            "a name with no value counts as absent"
        );
    }

    #[test]
    fn every_locale_gets_the_same_name() {
        // The product name is one string in all seven languages; a locale left holding the
        // placeholder would show it to a user.
        let mut raw = raw_with(APP_NAME_PLACEHOLDER);

        replace_placeholders(&mut raw, &[(APP_NAME_PLACEHOLDER, "Neutral".to_string())]);

        for locale in ["en", "nl"] {
            assert_eq!(raw.maps[locale]["app_title"], "Neutral");
            assert_eq!(
                raw.maps[locale]["nav_mail"], "Mail",
                "other messages are untouched"
            );
        }
    }

    #[test]
    fn the_placeholder_is_replaced_in_place_not_only_alone() {
        let mut raw = raw_with("Welcome to {app_name}");

        replace_placeholders(&mut raw, &[(APP_NAME_PLACEHOLDER, "Neutral".to_string())]);

        assert_eq!(raw.maps["en"]["app_title"], "Welcome to Neutral");
    }

    #[test]
    fn every_placeholder_is_filled_in_one_message() {
        let mut raw = raw_with("{app_name}: {privacy_url}");

        replace_placeholders(
            &mut raw,
            &[
                (APP_NAME_PLACEHOLDER, "Neutral".to_string()),
                ("{privacy_url}", "https://example.org/privacy".to_string()),
            ],
        );

        assert_eq!(
            raw.maps["en"]["app_title"],
            "Neutral: https://example.org/privacy"
        );
    }

    #[test]
    fn the_neutral_default_fills_every_placeholder() {
        // A placeholder whose variable `default.env` does not name would reach a user verbatim in
        // every unbranded build.
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let defaults = parse(
            &std::fs::read_to_string(root.join("branding/default.env"))
                .expect("every checkout has the neutral default"),
        );
        for (placeholder, key) in PLACEHOLDERS {
            assert!(defaults.contains_key(key), "{placeholder} needs {key}");
        }
    }

    #[test]
    fn the_brand_file_wins_over_the_default_and_the_environment_over_both() {
        let dir = scratch("mailcal-l10n-brand-order");
        write(
            &dir,
            "default.env",
            "MAILCAL_APP_NAME=Neutral\nMAILCAL_APP_ID=org.neutral.client\n",
        );

        assert_eq!(
            value(&dir, APP_NAME_KEY, None).expect("the default names it"),
            "Neutral"
        );

        write(&dir, "allodia.env", "MAILCAL_APP_NAME=Older\n");

        assert_eq!(
            value(&dir, APP_NAME_KEY, None).expect("the older name still brands"),
            "Older"
        );

        write(&dir, "brand.env", "MAILCAL_APP_NAME=Branded\n");

        assert_eq!(
            value(&dir, APP_NAME_KEY, None).expect("the brand file wins over the older name"),
            "Branded"
        );
        assert_eq!(
            value(&dir, APP_NAME_KEY, Some(" Typed ")).expect("the caller wins over both"),
            "Typed"
        );
        assert_eq!(
            value(&dir, APP_NAME_KEY, Some("  ")).expect("blank is not a name"),
            "Branded",
            "an empty variable is the shape CI leaves behind, and must not win"
        );
        // A brand file that names only some keys leaves the rest on the default, rather than
        // taking them away: the two files are merged, not chosen between.
        assert_eq!(
            value(&dir, APP_ID_KEY, None).expect("the default still answers for the id"),
            "org.neutral.client"
        );
    }

    #[test]
    fn the_variable_that_answers_first_is_the_one_a_rebuild_watches() {
        // The variable is the one that has to be watched: `scripts/dev/lib.sh` exports the
        // identity before it builds anything, so on a scripted build the environment answers and
        // no brand file is read. A checkout that gains `branding/brand.env` would otherwise go
        // on generating the neutral catalog, which `scripts/dev/test-linux-ui.sh` sees as the
        // branded window never appearing.
        let root = Path::new("/checkout");
        let lines = tracking(root, APP_NAME_KEY);

        assert!(
            lines.contains(&format!("cargo:rerun-if-env-changed={APP_NAME_KEY}")),
            "{lines:?}"
        );
        // Joined rather than spelled out: a path separator is the host's, so a literal
        // "/checkout/branding" is a Windows-only failure of a rule that holds on every host.
        for file in [
            "branding",
            "branding/brand.env",
            "branding/allodia.env",
            "branding/default.env",
        ] {
            let watched = file
                .split('/')
                .fold(root.to_path_buf(), |path, part| path.join(part));
            assert!(
                lines.contains(&format!("cargo:rerun-if-changed={}", watched.display())),
                "{file} is not watched: {lines:?}"
            );
        }
    }

    #[test]
    fn a_checkout_without_the_default_is_an_error_not_an_empty_name() {
        let dir = scratch("mailcal-l10n-brand-missing");

        let err = value(&dir, APP_NAME_KEY, None).expect_err("nothing names the app");

        assert!(err.contains("MAILCAL_APP_NAME"), "{err}");
    }

    #[test]
    fn the_user_agent_names_the_product_as_one_lowercase_token() {
        assert_eq!(
            product_token("Allodia Mail & Calendar").as_deref(),
            Some("allodia-mail-calendar")
        );
        assert_eq!(product_token("MailCal").as_deref(), Some("mailcal"));
        assert_eq!(product_token("  Été 2 Go!").as_deref(), Some("t-2-go"));
        assert_eq!(
            product_token("Почта"),
            None,
            "nothing is left to name it by"
        );
    }
}
