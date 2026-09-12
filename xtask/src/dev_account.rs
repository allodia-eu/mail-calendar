//! Guards the dev-account (local Stalwart harness) contract.
//!
//! `MAILCAL_DEV_ACCOUNT` swaps the developer's real mailbox for a throwaway loopback one, and its
//! IMAP mode additionally teaches the core to trust a self-signed certificate. Both properties fail
//! *silently* when they break: a drifted fixture just fails to log in, and a trust path that leaks
//! into a shipped build looks like nothing at all.

use std::path::Path;

use crate::{
    git::{self, Kind},
    report::Report,
};

/// The four clients that inject the harness IMAP account.
///
/// Named by directory rather than by file. The fields have to exist in the client; which file holds
/// them is not the invariant, and splitting one for the 500-line limit has already moved them once.
const IMAP_CLIENTS: &[&str] = &[
    "clients/apple/Packages/MailcalKit/Sources/MailcalUI",
    "clients/android/app/src/main/java/eu/allodia/mailcal",
    "clients/windows/Mailcal/Services",
    "clients/linux/src",
];

/// The hand-written `[imap]` config each of them carries.
///
/// The config builder cannot produce it (it always derives `server_name` from the dialled host,
/// and the harness dials by IP while its certificate's only SAN is `localhost`). So the fixture is
/// duplicated by necessity, in four languages, and a value edited in one place fails to log in from
/// exactly one platform, with a plain "authentication failed", nowhere near the edit.
const IMAP_FIELDS: &[&str] = &[
    r#"addr = "127.0.0.1:12993""#,
    r#"server_name = "localhost""#,
    r#"username = "alice@test.local""#,
    r#"password = "harness-alice-pw""#,
];

/// The two values the harness itself is built around: `lib.sh` dials the listener and seeds alice's
/// password, and a port or password changed there must reach the clients' fixtures.
const HARNESS_VALUES: &[&str] = &["127.0.0.1:12993", "harness-alice-pw"];

/// Runs the check. `Ok(true)` means the contract holds.
///
/// # Errors
///
/// Propagates a git failure.
pub(crate) fn run(root: &Path) -> Result<bool, String> {
    let mut report = Report::new();

    fixture_in_step(root, &mut report)?;
    harness_dials_the_same_listener(root, &mut report)?;
    switch_is_debug_only(root, &mut report)?;
    trust_anchor_excluded_from_release(root, &mut report);

    if report.failed() {
        report.emit();
        return Ok(false);
    }
    println!(
        "OK: the dev-account contract holds (IMAP fixture in step, switch is debug-only, harness \
         CA trust excluded from release)."
    );
    Ok(true)
}

/// Every client carries every field.
///
/// One search per field over all four directories at once, rather than one per pair. Sixteen
/// searches was sixteen processes, and on a host where a process costs a tenth of a second that is
/// most of what this check spends.
fn fixture_in_step(root: &Path, report: &mut Report) -> Result<(), String> {
    for field in IMAP_FIELDS {
        let hits = git::grep(root, Kind::Fixed, field, IMAP_CLIENTS)?;
        let files = hits.files();
        for client in IMAP_CLIENTS {
            if !files.iter().any(|f| f.starts_with(client)) {
                report.note(format!(
                    "nothing in {client} carries the harness IMAP fixture field:\n    {field}"
                ));
                report.note(
                    "All four clients inject the same hand-written [imap] config; a drift here \
                     logs in from every platform but one, and says only \"authentication failed\".",
                );
            }
        }
    }
    Ok(())
}

/// The harness and the injected fixture must dial the same listener with the same password.
fn harness_dials_the_same_listener(root: &Path, report: &mut Report) -> Result<(), String> {
    let lib = git::read(root, "scripts/dev/lib.sh")?;
    for value in HARNESS_VALUES {
        if !lib.contains(value) {
            report.note(format!(
                "scripts/dev/lib.sh no longer carries {value}, which the clients hard-code."
            ));
        }
    }
    Ok(())
}

/// Every client compiles the switch out of a release build, so a shipped binary can never open the
/// harness mailbox because of a stray environment variable.
///
/// Apple and Windows use `#if DEBUG`; Android decodes the mode only when `FLAG_DEBUGGABLE` is set;
/// Linux compiles the fixture away entirely.
fn switch_is_debug_only(root: &Path, report: &mut Report) -> Result<(), String> {
    for file in [
        "clients/apple/Packages/MailcalKit/Sources/MailcalUI/MailcalModel+DevAccount.swift",
        "clients/windows/Mailcal/Services/MailboxModel.Accounts.cs",
    ] {
        if !holds(root, file, "#if DEBUG") {
            report.note(format!(
                "{file} has no `#if DEBUG` guard around the dev-account switch."
            ));
            report.note("A release build must ignore MAILCAL_DEV_ACCOUNT entirely.");
        }
    }

    let android = "clients/android/app/src/main/java/eu/allodia/mailcal";
    if git::grep(root, Kind::Fixed, "FLAG_DEBUGGABLE", &[android])?.is_empty() {
        report.note("the Android dev-account switch is no longer gated on FLAG_DEBUGGABLE.");
        report.note("A release build must ignore MAILCAL_DEV_ACCOUNT entirely.");
    }

    if !holds(
        root,
        "clients/linux/src/dev_account.rs",
        "#![cfg(debug_assertions)]",
    ) {
        report.note(
            "the Linux dev-account fixture is not compiled only under `debug_assertions`. A \
             release build must ignore MAILCAL_DEV_ACCOUNT entirely.",
        );
    }
    Ok(())
}

/// The IMAP mode's extra trust anchor stays out of production.
///
/// `dev_tls::extra_ca_anchors` folds the PEM named by `MAILCAL_EXTRA_CA` into the account's TLS
/// roots; a shipped binary that kept it would trust any CA an attacker could place on disk and name
/// in the environment. Two things hold it out: the `cfg(not(...))` arm in `tls.rs`, and
/// `dev-harness` never being a default feature.
fn trust_anchor_excluded_from_release(root: &Path, report: &mut Report) {
    let tls = "crates/mailcal-account/src/tls.rs";
    let guard = "#[cfg(not(any(debug_assertions, feature = \"dev-harness\")))]";
    if !holds(root, tls, guard) {
        report.note(format!(
            "{tls} no longer excludes the extra-CA loader from production builds. `custom_roots()` \
             must be an empty Vec unless debug_assertions or the `dev-harness` feature is on, or a \
             shipped binary trusts whatever MAILCAL_EXTRA_CA names."
        ));
    }

    for manifest in [
        "crates/mailcal-account/Cargo.toml",
        "crates/mailcal-bindings/Cargo.toml",
    ] {
        let text = git::read(root, manifest).unwrap_or_default();
        if text.lines().any(|l| defaults_to(l, "dev-harness")) {
            report.note(format!(
                "{manifest} enables `dev-harness` by default. It must stay opt-in (the Android dev \
                 loop passes --features dev-harness); on by default it would compile the harness \
                 CA trust path into a shipped release binary."
            ));
        }
    }
}

/// True when `file` exists and carries `needle`.
///
/// A file that cannot be read answers false, which is the direction that reports the problem: a
/// guard nobody can find is a guard that is not there.
fn holds(root: &Path, file: &str, needle: &str) -> bool {
    std::fs::read_to_string(root.join(file)).is_ok_and(|text| text.contains(needle))
}

/// True for a `default = [ … feature … ]` line, which is `^default *= *\[.*<feature>`.
fn defaults_to(line: &str, feature: &str) -> bool {
    let Some(rest) = line.strip_prefix("default") else {
        return false;
    };
    let rest = rest.trim_start_matches(' ');
    let Some(rest) = rest.strip_prefix('=') else {
        return false;
    };
    let rest = rest.trim_start_matches(' ');
    rest.strip_prefix('[')
        .is_some_and(|list| list.contains(feature))
}

#[cfg(test)]
mod tests {
    use super::defaults_to;

    #[test]
    fn spots_a_default_enabled_feature() {
        assert!(defaults_to("default = [\"dev-harness\"]", "dev-harness"));
        assert!(defaults_to(
            "default=[\"a\", \"dev-harness\"]",
            "dev-harness"
        ));
    }

    #[test]
    fn an_opt_in_feature_is_fine() {
        assert!(!defaults_to("default = []", "dev-harness"));
        assert!(!defaults_to("dev-harness = []", "dev-harness"));
    }

    #[test]
    fn an_indented_key_is_not_the_table_default() {
        // The rule is anchored: a `default` nested inside some other value is not the feature list.
        assert!(!defaults_to("  default = [\"dev-harness\"]", "dev-harness"));
    }
}
