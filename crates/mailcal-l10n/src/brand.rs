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
//! Both come from the same two files every other build system reads, by the same rule.

use std::{collections::BTreeMap, path::Path};

use crate::model::Raw;

/// The placeholder the catalog uses for the product's own name.
const APP_NAME_PLACEHOLDER: &str = "{app_name}";

/// The variables that carry the identity, in the environment and in the brand files alike.
const APP_NAME_KEY: &str = "MAILCAL_APP_NAME";
const APP_ID_KEY: &str = "MAILCAL_APP_ID";

/// The identity this build carries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Brand {
    /// What the app calls itself, in every language.
    pub name: String,
    /// The reverse-DNS id the OS knows it by, which is also a URI scheme.
    pub id: String,
}

/// Resolves the identity: the environment, then Allodia's brand file when the checkout has one,
/// then the neutral default that every checkout has.
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
    })
}

/// Replaces [`APP_NAME_PLACEHOLDER`] in every message of every locale.
pub(crate) fn replace_app_name(raw: &mut Raw, name: &str) {
    for messages in raw.maps.values_mut() {
        for template in messages.values_mut() {
            if template.contains(APP_NAME_PLACEHOLDER) {
                *template = template.replace(APP_NAME_PLACEHOLDER, name);
            }
        }
    }
}

fn value(root: &Path, key: &str, from_environment: Option<&str>) -> Result<String, String> {
    if let Some(found) = from_environment.map(str::trim).filter(|v| !v.is_empty()) {
        return Ok(found.to_string());
    }
    let branding = root.join("branding");
    for file in ["allodia.env", "default.env"] {
        // Emitted whether or not the file exists, and before the read, because the interesting
        // change is a brand file **appearing**: a build tree that has already generated an
        // unbranded catalog keeps serving it otherwise, and the app carries the neutral name with
        // nothing to say it should not. Cargo treats a `rerun-if-changed` on an absent path as
        // "rerun when it appears", which is exactly the case that was silent.
        //
        // Harmless outside a build script: the line goes to stdout, which only Cargo reads.
        println!("cargo:rerun-if-changed={}", branding.join(file).display());
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
            .strip_prefix('"')
            .and_then(|v| v.strip_suffix('"'))
            .or_else(|| value.strip_prefix('\'').and_then(|v| v.strip_suffix('\'')))
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

    use super::{APP_ID_KEY, APP_NAME_KEY, APP_NAME_PLACEHOLDER, parse, replace_app_name, value};
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

        replace_app_name(&mut raw, "Neutral");

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

        replace_app_name(&mut raw, "Neutral");

        assert_eq!(raw.maps["en"]["app_title"], "Welcome to Neutral");
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

        write(&dir, "allodia.env", "MAILCAL_APP_NAME=Branded\n");

        assert_eq!(
            value(&dir, APP_NAME_KEY, None).expect("the brand file wins"),
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
    fn a_checkout_without_the_default_is_an_error_not_an_empty_name() {
        let dir = scratch("mailcal-l10n-brand-missing");

        let err = value(&dir, APP_NAME_KEY, None).expect_err("nothing names the app");

        assert!(err.contains("MAILCAL_APP_NAME"), "{err}");
    }
}
