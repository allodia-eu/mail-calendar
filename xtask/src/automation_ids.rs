//! Fails when a Windows UI suite looks up an AutomationId the app no longer gives anything.
//!
//! The suites under `clients/windows/uitests` find what they drive by AutomationId, and a lookup
//! for an id nothing carries returns nothing rather than failing: the case reads as the app
//! misbehaving or, where it asserts that something is gone, passes. Most of the suites need the
//! local mail harness, which CI cannot start (`run-ui-tests.ps1` says why), so a renamed id stays
//! unnoticed until someone runs them by hand. The trap that hides it best: giving an element an
//! explicit AutomationId renames it, because UI Automation reported its `x:Name` until then.
//!
//! An id the app has is one of:
//!
//! * in XAML, an element's `AutomationProperties.AutomationId`, or its `x:Name` while it has none;
//! * in C#, any string literal, since ids reach `SetAutomationId` through helpers' parameters; the
//!   text before the first hole of an interpolated literal, for an id assembled per item; or a name
//!   with a literal appended inside `SetAutomationId`;
//! * one the platform draws, listed in [`PLATFORM`] with where it comes from.
//!
//! A lookup is a quoted id after `-AutomationId`, after `.AutomationId -eq`, `-ne` or `-in`, or as
//! the first argument of a suite helper whose first parameter is `$AutomationId` or `$Id`; in the
//! C# suites (`clients/windows/Mailcal.UiTests`), a quoted id passed to `Filter.ById` or set as a
//! filter's `Id`. An id held in a variable is not followed, so a suite looking one up keeps it
//! quoted at the lookup.

use std::{collections::BTreeSet, path::Path};

use crate::{
    automation_ids_scan::{
        CsLiterals, Wanted, cs_literals, cs_lookups, id_helpers, lookups, xaml_ids,
    },
    git,
    report::Report,
};

/// The Windows app's sources.
const APP: &str = "clients/windows/Mailcal";

/// Where the suites and the helpers they share live.
const SUITES: &str = "clients/windows";

/// The C# suites, which reach the app through its test agent.
const CS_SUITES: &str = "clients/windows/Mailcal.UiTests";

/// Ids the suites look up that no source here assigns, each drawn by WinUI or Windows itself.
///
/// An entry no suite looks up any more fails the check too, so the list cannot outlive its reason.
pub(crate) const PLATFORM: &[(&str, &str)] = &[
    ("CloseButton", "a ContentDialog template part"),
    ("PrimaryButton", "a ContentDialog template part"),
    ("SecondaryButton", "a ContentDialog template part"),
    ("PART_PaneToggleButton", "the WinUI TitleBar's pane toggle"),
    ("TogglePaneButton", "the NavigationView's pane toggle"),
    ("SettingsItem", "the NavigationView's settings item"),
    ("TextBox", "the AutoSuggestBox's inner text box"),
    ("printerSelector", "the Windows print dialog"),
    ("PrintButton", "the Windows print dialog"),
    ("NoPreviewAvailableText", "the Windows print dialog"),
    ("PART_TitleText", "the WinUI TitleBar's title"),
    ("FileNameControlHost", "the Windows save dialog"),
    ("1001", "the Windows save dialog's file name box"),
];

/// Every id the app can give an element.
#[derive(Debug, Default)]
pub(crate) struct Known {
    /// Ids carried whole.
    exact: BTreeSet<String>,
    /// Starts of ids assembled per item.
    prefixes: Vec<String>,
    /// Endings appended to another id.
    suffixes: Vec<String>,
}

impl Known {
    /// Whether some element can carry what `wanted` looks up.
    fn has(&self, wanted: &Wanted) -> bool {
        match wanted {
            Wanted::Exact(id) => {
                self.exact.contains(id)
                    || self
                        .prefixes
                        .iter()
                        .any(|prefix| id.len() > prefix.len() && id.starts_with(prefix.as_str()))
                    || self.suffixes.iter().any(|suffix| {
                        id.strip_suffix(suffix.as_str())
                            .is_some_and(|stem| self.exact.contains(stem))
                    })
            }
            Wanted::Prefix(start) => {
                self.exact.iter().any(|id| id.starts_with(start.as_str()))
                    || self.prefixes.iter().any(|prefix| {
                        prefix.starts_with(start.as_str()) || start.starts_with(prefix.as_str())
                    })
            }
        }
    }
}

/// Runs the check. `Ok(true)` means every lookup names an id the app or the platform has.
///
/// # Errors
///
/// Fails when git cannot list the tree, or when either side reads as empty: no ids, or no lookups,
/// is a shape that moved, and a check comparing nothing to nothing cannot fail.
pub(crate) fn run(root: &Path) -> Result<bool, String> {
    check(root, PLATFORM)
}

/// [`run`], against a given platform list, so a fixture can bring its own.
pub(crate) fn check(root: &Path, platform: &[(&str, &str)]) -> Result<bool, String> {
    let known = app_ids(root)?;
    if known.exact.is_empty() {
        return Err(format!(
            "no AutomationIds found under {APP}: did the app move?"
        ));
    }

    let listed = git::listed(root, &[SUITES])?;
    let suites: Vec<String> = listed
        .iter()
        .filter(|name| crate::prose::extension(name) == "ps1")
        .cloned()
        .collect();
    let cs_suites: Vec<String> = listed
        .iter()
        .filter(|name| name.starts_with(CS_SUITES) && crate::prose::extension(name) == "cs")
        .cloned()
        .collect();
    let texts: Vec<(String, String)> = suites
        .into_iter()
        .map(|name| git::read(root, &name).map(|text| (name, text)))
        .collect::<Result<_, _>>()?;
    let cs_texts: Vec<(String, String)> = cs_suites
        .into_iter()
        .map(|name| git::read(root, &name).map(|text| (name, text)))
        .collect::<Result<_, _>>()?;
    let helpers: Vec<String> = texts
        .iter()
        .flat_map(|(_, text)| id_helpers(text))
        .collect();

    let mut report = Report::new();
    let mut used: BTreeSet<&str> = BTreeSet::new();
    let mut count = 0usize;
    let found = texts
        .iter()
        .flat_map(|(name, text)| {
            lookups(text, &helpers)
                .into_iter()
                .map(move |hit| (name, hit))
        })
        .chain(
            cs_texts
                .iter()
                .flat_map(|(name, text)| cs_lookups(text).into_iter().map(move |hit| (name, hit))),
        );
    for (name, (line, wanted)) in found {
        count += 1;
        if let Wanted::Exact(id) = &wanted
            && let Some((platform_id, _)) = platform.iter().find(|(p, _)| p == id)
        {
            used.insert(platform_id);
            continue;
        }
        if !known.has(&wanted) {
            report.note(match &wanted {
                Wanted::Exact(id) => {
                    format!("{name}:{line}: '{id}' is on no element the app draws")
                }
                Wanted::Prefix(start) => {
                    format!("{name}:{line}: no id the app draws starts with '{start}'")
                }
            });
        }
    }
    if count == 0 {
        return Err(format!(
            "no AutomationId lookups found under {SUITES}: did the suites move?"
        ));
    }
    for (id, from) in platform {
        if !used.contains(id) {
            report.note(format!(
                "xtask/src/automation_ids.rs: PLATFORM lists '{id}' ({from}), which no suite looks up"
            ));
        }
    }

    if !report.failed() {
        println!(
            "Windows UI suites: {count} AutomationId lookups, each on an element the app or Windows draws."
        );
        return Ok(true);
    }
    eprintln!("Windows UI suites look up AutomationIds nothing carries:\n");
    report.emit();
    eprintln!(
        "\nAn element given an explicit AutomationId no longer answers to its x:Name. Point the suite at\n\
         the id the app sets now, or add a platform id to PLATFORM with where it comes from."
    );
    Ok(false)
}

/// Every id the app's XAML and C# can give an element.
fn app_ids(root: &Path) -> Result<Known, String> {
    let mut known = Known::default();
    let mut literals = CsLiterals::default();
    for name in git::listed(root, &[APP])? {
        match crate::prose::extension(&name) {
            "xaml" => known.exact.extend(xaml_ids(&git::read(root, &name)?)),
            "cs" => cs_literals(&git::read(root, &name)?, &mut literals),
            _ => {}
        }
    }
    known.exact.extend(literals.exact);
    known.prefixes = literals.prefixes;
    known.suffixes = literals.suffixes;
    Ok(known)
}
