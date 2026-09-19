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
        if hits.is_empty() || entitlements.contains(rule.key) {
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
