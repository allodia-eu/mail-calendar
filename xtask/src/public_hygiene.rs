//! Fails when something the public repository must not carry has come back.
//!
//! This tree is the published one, so the rule is not a one-off strip before a copy: a tracker
//! reference or a personal identifier written today reaches a reader tomorrow. This is the machine
//! half, the patterns a search can decide, held at zero.
//!
//! What it deliberately does not decide is prose. Whether a paragraph is competitor commentary,
//! whether a design note may cite another mail client, whether a document belongs in the public
//! tree at all: those need a reading, and they are settled per file, not per pattern.

use std::path::Path;

use crate::{
    git::{self, Kind, Matches},
    report::Report,
};

/// Paths this check does not read. Each is either not maintained prose (history, vendored upstream,
/// a generated bundle) or a file that has to hold the shapes forbidden below in order to forbid
/// them. The list is here rather than anywhere else because it is what a reader wants in front of
/// them when a match looks like a false positive.
const EXCLUDED: &[&str] = &[
    ":!docs/*-plan.md",
    ":!docs/changelog/released",
    ":!clients/android/gradlew",
    ":!clients/composer/dist",
    ":!.agents/skills/gh-stack",
    // The patterns themselves, and the fixtures written to trip them.
    ":!xtask/src/public_hygiene.rs",
    ":!xtask/src/fixture_tests.rs",
];

/// `#NNN` addresses an issue in a repository the public tree has no link to, so the reference is a
/// dead end for every reader it reaches. The repository's own writing rule covers the same ground
/// from the other side: a comment says what the code does now, not which change landed it.
const ISSUE_SHAPE: &str =
    r"(\(#[0-9]{2,4}\)|issue #[0-9]+|[Pp][Rr] #[0-9]+|\[#[0-9]{2,4}\]|→ #[0-9]+)";

/// A pointer at a repository the reader cannot open fails twice: they cannot open it, and what it
/// says goes stale on a schedule nothing here can see. Describe the thing by what it does instead.
/// Naming the *shape* of the pointer rather than the repository is what keeps this rule
/// publishable.
///
/// The article is the signal, and it is why this can be a search at all. *The* release repository
/// and *our* private one point somewhere; *a* private repository is a category in an argument,
/// which is how `docs/pledge.md` promises no build step reaches one and how
/// `allodia_license/README.md` explains why a closed component is worse than a readable one. Those
/// stay.
const PRIVATE_POINTER: &str = "(the|our) (release|internal|private) repos(itory)?|[Hh]andbook";

/// "Phase A", "the field-parity wave" name a step in a roadmap. Two things are wrong with one in
/// the code: it points at a document the reader may not have, and it stops being true the moment
/// the plan moves.
///
/// Capitalised and a single token, because a plan phase is a proper noun. The domain's own uses, a
/// gesture's propagation phase or an Xcode build phase, are lowercase and stay untouched, which is
/// why the pattern is not case-insensitive. The boundaries are spelled out rather than written
/// `\b`: this is a POSIX extended regular expression, where `\b` is not a word boundary and the
/// pattern would silently match nothing.
const PHASE_SHAPE: &str = "(Phase[ -][A-Z0-9]([^A-Za-z0-9]|$)|the [a-z-]+ wave([^a-z]|$))";

/// Runs the check. `Ok(true)` means the tree carries nothing the public copy must not.
///
/// # Errors
///
/// Propagates a git failure.
pub(crate) fn run(root: &Path) -> Result<bool, String> {
    let mut report = Report::new();

    // The engine's tracker is public, so a line naming that repository may carry a number; #1234
    // and #1281 are the designated stand-ins, for the few places that have to show an issue-shaped
    // token in order to forbid one.
    let tracker = search(root, ISSUE_SHAPE, &[])?
        .without_lines_containing("email-calendar-sync-engine")
        .without(is_a_standin_issue);
    record(
        &tracker,
        "a reference to the private tracker (state the rule, not the ticket):",
        &mut report,
    );

    record(
        &search(root, PRIVATE_POINTER, &[])?,
        "a pointer at a repository the reader cannot open (say what it does, not where it is):",
        &mut report,
    );

    // The roadmap that defines the phases is excluded from the tree outright, above. Nothing that
    // ships is exempt: a finished plan is deleted, not carried and excused.
    record(
        &search(root, PHASE_SHAPE, &[])?,
        "a plan phase named outside the roadmap (state the fact, not the phase):",
        &mut report,
    );

    // Fixtures and docs name no individual. The four documents that need a party name one on
    // purpose: an eenmanszaak has no legal personality of its own, so the brand alone would name
    // nobody a licence could bind.
    record(
        &search(
            root,
            "(Dennis|Ameling|dennisameling)",
            &[
                ":!CLA.md",
                ":!REUSE.toml",
                ":!allodia_license/LICENSE.md",
                ":!LICENSES/LicenseRef-*",
            ],
        )?,
        "a personal identifier in a fixture, doc or script:",
        &mut report,
    );
    // A public repository carrying these can put something in a store under this product's name.
    record(
        &search(root, "(X98DRMUM3J|947BB2P68Y|Fits4all)", &[])?,
        "an Apple team reservation:",
        &mut report,
    );
    record(
        &search(root, r"allodia\.e2e", &[])?,
        "an internal test account:",
        &mut report,
    );

    if report.failed() {
        report.emit();
        eprintln!("ERROR: the tree carries something the public repository must not.");
        return Ok(false);
    }

    println!("OK: no private references, plan phases, personal identifiers or store reservations.");
    Ok(true)
}

/// Searches the whole tree minus [`EXCLUDED`], plus any extra pathspecs.
fn search(root: &Path, pattern: &str, extra: &[&str]) -> Result<Matches, String> {
    let mut pathspecs: Vec<&str> = vec!["."];
    pathspecs.extend_from_slice(EXCLUDED);
    pathspecs.extend_from_slice(extra);
    git::grep(root, Kind::Extended, pattern, &pathspecs)
}

/// Records `what` and the offending lines, when there are any.
fn record(hits: &Matches, what: &str, report: &mut Report) {
    if hits.is_empty() {
        return;
    }
    report.note(what);
    for line in &hits.lines {
        report.note(format!("  {line}"));
    }
}

/// True when the only issue-shaped tokens on the line are the designated stand-ins.
///
/// Matches `grep -vE '#(1234|1281)([^0-9]|$)'`: the line is dropped when it carries one of those
/// numbers not followed by a further digit, so `#12345` is still a real reference.
fn is_a_standin_issue(line: &str) -> bool {
    for standin in ["#1234", "#1281"] {
        let mut from = 0;
        while let Some(at) = line[from..].find(standin) {
            let end = from + at + standin.len();
            if !line[end..].starts_with(|c: char| c.is_ascii_digit()) {
                return true;
            }
            from = end;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::is_a_standin_issue;

    #[test]
    fn a_standin_is_dropped() {
        assert!(is_a_standin_issue("see (#1234) for the shape"));
        assert!(is_a_standin_issue("and #1281."));
    }

    #[test]
    fn a_longer_number_starting_with_a_standin_is_still_a_reference() {
        // `#12345` is a real ticket, not the stand-in, so the line must survive the filter.
        assert!(!is_a_standin_issue("see #12345"));
    }

    #[test]
    fn an_ordinary_reference_is_kept() {
        assert!(!is_a_standin_issue("closes issue #77"));
    }
}
