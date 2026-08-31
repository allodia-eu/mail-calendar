//! Loads the inlang project settings and per-locale message catalogs from disk into a
//! [`Raw`]. The on-disk layout mirrors the Allodia web app: `project.inlang/settings.json`
//! names the base locale + locale list, and `messages/<locale>.json` holds a flat
//! `key -> template` object (with `$schema` and other `$`-prefixed meta keys ignored).

use std::{collections::BTreeMap, path::Path};

use serde_json::Value;

use crate::model::Raw;

/// Loads `project.inlang/settings.json` and `messages/<locale>.json` under `root`.
///
/// # Errors
///
/// Returns a human-readable message if a file is missing, is not valid JSON, lacks the
/// expected `baseLocale` / `locales` fields, or holds a non-string message value.
pub(crate) fn load(root: &Path) -> Result<Raw, String> {
    let settings_path = root.join("project.inlang/settings.json");
    let settings = read_json(&settings_path)?;

    let base = settings
        .get("baseLocale")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("{}: missing string \"baseLocale\"", settings_path.display()))?
        .to_string();
    let locales: Vec<String> = settings
        .get("locales")
        .and_then(Value::as_array)
        .ok_or_else(|| format!("{}: missing array \"locales\"", settings_path.display()))?
        .iter()
        .filter_map(|v| v.as_str().map(str::to_string))
        .collect();
    if !locales.iter().any(|l| l == &base) {
        return Err(format!(
            "{}: baseLocale \"{base}\" is not in locales {locales:?}",
            settings_path.display()
        ));
    }

    let mut maps = BTreeMap::new();
    for loc in &locales {
        let path = root.join(format!("messages/{loc}.json"));
        let obj = read_json(&path)?;
        let entries = obj
            .as_object()
            .ok_or_else(|| format!("{}: expected a JSON object", path.display()))?;
        let mut map = BTreeMap::new();
        for (key, value) in entries {
            if key.starts_with('$') {
                continue; // $schema and friends
            }
            let template = value.as_str().ok_or_else(|| {
                format!("{}: value for \"{key}\" is not a string", path.display())
            })?;
            map.insert(key.clone(), template.to_string());
        }
        maps.insert(loc.clone(), map);
    }

    Ok(Raw {
        base,
        locales,
        maps,
    })
}

fn read_json(path: &Path) -> Result<Value, String> {
    let body = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    serde_json::from_str(&body).map_err(|e| format!("{}: {e}", path.display()))
}
