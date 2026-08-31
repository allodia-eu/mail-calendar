//! Validates a [`Raw`] catalog before any code is emitted. This is the cross-platform
//! "type safety" guard: it fails the build (rather than silently emitting a broken table)
//! when a translation is missing, has an extra key, disagrees on a key's placeholders, or
//! uses a key that is not a legal identifier. Every problem is collected so one run reports
//! them all.

use crate::model::{Raw, is_identifier, placeholders};

/// Checks the catalog, returning every problem joined into one error string, or `Ok(())`
/// when the catalog is sound.
///
/// # Errors
///
/// Returns the joined problem list when a non-base locale is missing or has extra keys, a
/// key's placeholder set differs from the base, or a base key is not a valid identifier.
pub(crate) fn check(raw: &Raw) -> Result<(), String> {
    let mut problems: Vec<String> = Vec::new();
    let base_map = raw
        .maps
        .get(&raw.base)
        .ok_or_else(|| format!("base locale \"{}\" has no message file", raw.base))?;

    // Keys must be identifier-safe so they map to legal Swift/Kotlin/C# symbols.
    for key in base_map.keys() {
        if !is_identifier(key) {
            problems.push(format!(
                "[{}] key \"{key}\" is not a valid identifier",
                raw.base
            ));
        }
    }

    // Every shipped locale needs its own name in the picker. The clients render the language
    // list from the catalog (`L10n.locales` + `languageName`), so a locale without a
    // `settings_language_<loc>` message would show up there as an unlabelled row.
    for loc in &raw.locales {
        let key = format!("settings_language_{loc}");
        if !base_map.contains_key(&key) {
            problems.push(format!(
                "[{}] missing key \"{key}\"; every locale needs its own picker label",
                raw.base
            ));
        }
    }

    // Every non-base locale must define exactly the base key set.
    for loc in &raw.locales {
        if loc == &raw.base {
            continue;
        }
        let map = &raw.maps[loc];
        for key in base_map.keys() {
            if !map.contains_key(key) {
                problems.push(format!("[{loc}] missing key \"{key}\""));
            }
        }
        for key in map.keys() {
            if !base_map.contains_key(key) {
                problems.push(format!(
                    "[{loc}] unknown key \"{key}\" (not in base \"{}\")",
                    raw.base
                ));
            }
        }
    }

    // Each shared key's placeholder set must match the base (order may differ).
    for (key, base_value) in base_map {
        let mut base_ph = placeholders(base_value);
        base_ph.sort();
        for loc in &raw.locales {
            if loc == &raw.base {
                continue;
            }
            if let Some(value) = raw.maps[loc].get(key) {
                let mut ph = placeholders(value);
                ph.sort();
                if ph != base_ph {
                    problems.push(format!(
                        "[{loc}] key \"{key}\" placeholders {ph:?} differ from base {base_ph:?}"
                    ));
                }
            }
        }
    }

    if problems.is_empty() {
        Ok(())
    } else {
        problems.sort();
        Err(problems.join("\n"))
    }
}
