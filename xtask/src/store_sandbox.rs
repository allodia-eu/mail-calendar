//! Keeps the Mac App Store build's sandbox grants in step with what the Apple client does.
//!
//! Of the Apple signing shapes, only the Store build is sandboxed: the Developer ID `.dmg`, the
//! ad-hoc dev build and every test run outside it. So a capability the sandbox withholds behaves
//! perfectly everywhere a change is written and tested, compiles clean, signs clean, uploads
//! clean, and then returns an errno to the one user who installed it from the Store. That is not
//! hypothetical either: `GoogleLoopbackFlow` bound its redirect listener for months against an
//! entitlements file whose comment said the app never listens, and Google sign-in on the Store
//! build failed with "Operation not permitted" while the `.dmg` from the same commit signed in.
//!
//! Each rule below pairs something the client *does*, as it is written in Swift, with the
//! entitlement the sandbox demands for it. A grep is enough because the question is not how the
//! code is reached, only whether the capability is in the binary at all: nothing may ship a use
//! the Store build is not granted.

use std::path::Path;

use crate::git::{self, Kind};

/// The sandbox grants of the Mac App Store build.
const ENTITLEMENTS: &str = "clients/apple/App/AllodiaMail.appstore.entitlements";

/// Where a use of one of these capabilities could be written.
const SOURCES: &[&str] = &["clients/apple/**/*.swift"];

/// One capability, the grant it needs, and what a user sees without it.
struct Rule {
    /// How the capability is spelled in Swift.
    marker: &'static str,
    /// The entitlement the sandbox demands for it.
    key: &'static str,
    /// The feature that stops working, in the terms the reader is debugging it in.
    feature: &'static str,
    /// What the user gets instead.
    symptom: &'static str,
}

const RULES: &[Rule] = &[Rule {
    marker: "NWListener(",
    key: "com.apple.security.network.server",
    feature: "the Google sign-in redirect listener (GoogleOAuth.swift)",
    symptom: "the bind returns EPERM and the setup sheet reads \
              \"POSIXErrorCode(rawValue: 1): Operation not permitted\"",
}];

/// Whether the entitlements file actually **grants** `key`, rather than merely naming it.
///
/// Asking whether the text contains the key is not the same question. Every grant in that file
/// carries a comment explaining it, so the key's own name is prose there as well as markup, and a
/// grant written `<false/>` is a denial the sandbox enforces exactly as a missing key would. Either
/// would leave this check green over the build it exists to fail, so what counts is the `<key>`
/// element with `<true/>` as the next thing after it.
fn grants(entitlements: &str, key: &str) -> bool {
    let element = format!("<key>{key}</key>");
    entitlements.match_indices(&element).any(|(at, _)| {
        entitlements[at + element.len()..]
            .lines()
            .map(str::trim)
            .find(|line| !line.is_empty())
            .is_some_and(|line| line == "<true/>")
    })
}

/// Runs the check. `Ok(true)` means the Store build grants everything the client uses.
///
/// # Errors
///
/// Propagates a git failure, or an unreadable entitlements file. Both matter here: an error read
/// as "no matches" would pass this check on exactly the tree it exists to fail.
pub(crate) fn run(root: &Path) -> Result<bool, String> {
    let entitlements = git::read(root, ENTITLEMENTS)?;
    let mut failed = false;

    for rule in RULES {
        let hits = git::grep(root, Kind::Fixed, rule.marker, SOURCES)?;
        if hits.is_empty() || grants(&entitlements, rule.key) {
            continue;
        }
        println!(
            "ERROR: the Apple client uses {} but the Mac App Store build does not grant {}.",
            rule.marker, rule.key
        );
        for line in &hits.lines {
            println!("    {line}");
        }
        eprintln!(
            "
{} needs {} in {}.
Without it {}.

Only the Store build is sandboxed, so nothing else here can tell you: the Developer ID .dmg and
the dev build both work, and so does every test.",
            rule.feature, rule.key, ENTITLEMENTS, rule.symptom
        );
        failed = true;
    }

    if failed {
        return Ok(false);
    }

    println!(
        "OK: the Mac App Store build grants every sandboxed capability the Apple client uses."
    );
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::{ENTITLEMENTS, RULES, grants};

    #[test]
    fn a_key_set_true_is_a_grant() {
        assert!(grants(
            "<key>com.apple.security.network.server</key>\n    <true/>\n",
            "com.apple.security.network.server"
        ));
    }

    #[test]
    fn a_key_set_false_is_not_a_grant() {
        // The sandbox denies this exactly as it denies a missing key, so the check must too.
        assert!(!grants(
            "<key>com.apple.security.network.server</key>\n    <false/>\n",
            "com.apple.security.network.server"
        ));
    }

    #[test]
    fn a_key_named_only_in_a_comment_is_not_a_grant() {
        // Every grant in the file is explained beside it, so the key's name is prose there too.
        assert!(!grants(
            "<!-- com.apple.security.network.server is what the bind needs. -->\n<true/>\n",
            "com.apple.security.network.server"
        ));
    }

    #[test]
    fn the_shipped_entitlements_grant_every_rule() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("the xtask crate sits in the workspace root");
        let entitlements =
            std::fs::read_to_string(root.join(ENTITLEMENTS)).expect("the entitlements file");
        for rule in RULES {
            assert!(
                grants(&entitlements, rule.key),
                "{} is not granted",
                rule.key
            );
        }
    }
}
