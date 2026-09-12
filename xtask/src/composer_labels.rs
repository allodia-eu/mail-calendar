//! Fails when a client does not send the shared editor every label it has.
//!
//! The composer's chrome lives in one bundle shared by four clients, so its strings cannot be baked
//! per-language. Each client passes its own translations through `window.setComposerLabels`, and
//! `clients/composer/src/labels.ts` is the list of what there is to pass.
//!
//! Every way of getting this wrong is silent:
//!
//! * a client that never calls the hook keeps the bundle's built-in English, which is what macOS,
//!   iOS and Windows shipped for two releases with nothing in any build to say so;
//! * a key the bundle does not know is dropped by its `mergeLabels`;
//! * a key it knows but a client omits keeps that one control's English default, in an otherwise
//!   translated toolbar.
//!
//! None of the three throws, logs, or fails a build. They are visible only to someone running that
//! client in that language and looking at that control.
//!
//! Two things are checked per client: that it calls the hook at all, and that the key set it builds
//! is exactly the bundle's. What the strings *say* is not checked: codegen already fails the build
//! on a catalog key that does not exist, and a wrong translation is not a machine's call.

use std::path::Path;

use crate::git;

/// The list every client is measured against.
const BUNDLE: &str = "clients/composer/src/labels.ts";

/// The hook a client has to call.
const HOOK: &str = "setComposerLabels";

/// One client's half of the contract.
struct Client {
    /// What the report calls it.
    name: &'static str,
    /// Where it builds its label map.
    map: &'static str,
    /// What a quoted key looks like in that file: the text before it, and the text after it.
    key: (&'static str, &'static str),
    /// Where to look for the call, and what a call looks like there.
    ///
    /// Each is an *injection* and nothing else. Searching for the bare hook name does not work: it
    /// appears in the comment above every one of these maps, and on Apple and Windows in the
    /// helper that *builds* the script, so the check would be satisfied by the very file whose
    /// call sites had just been deleted.
    call: (&'static str, &'static str),
}

impl std::fmt::Debug for Client {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Client")
            .field("name", &self.name)
            .finish_non_exhaustive()
    }
}

/// The four clients, and how each spells the same dictionary.
const CLIENTS: &[Client] = &[
    Client {
        name: "android",
        map: "clients/android/app/src/main/java/eu/allodia/mailcal/ComposerEditorHost.kt",
        key: ("put(\"", "\""),
        call: (
            "clients/android/app/src/main/java/eu/allodia/mailcal",
            "window.setComposerLabels(",
        ),
    },
    Client {
        name: "apple",
        map: "clients/apple/Packages/MailcalKit/Sources/MailcalUI/ComposerLabels.swift",
        key: ("\"", "\": L10n."),
        call: (
            "clients/apple/Packages/MailcalKit/Sources/MailcalUI",
            "ComposerLabels.script()",
        ),
    },
    Client {
        name: "linux",
        map: "clients/linux/src/ui/composer.rs",
        key: ("\"", "\": l10n::"),
        call: ("clients/linux/src", "window.setComposerLabels("),
    },
    Client {
        name: "windows",
        map: "clients/windows/Mailcal/Services/ComposerLabels.cs",
        key: ("[\"", "\"] = L10n."),
        call: ("clients/windows/Mailcal", "ComposerLabels.Script()"),
    },
];

/// The source types a call site can be written in.
const SOURCES: &[&str] = &["kt", "swift", "rs", "cs"];

/// Runs the check. `Ok(true)` means every client sends the editor every label.
///
/// # Errors
///
/// Fails when the bundle cannot be read or its `Labels` interface cannot be found: a key list that
/// parsed to nothing would compare empty to empty and pass.
pub(crate) fn run(root: &Path) -> Result<bool, String> {
    let expected = bundle_keys(&git::read(root, BUNDLE)?)?;
    let mut failures: Vec<String> = Vec::new();

    for client in CLIENTS {
        match git::read(root, client.map) {
            Err(_) => failures.push(format!(
                "{}: {} is missing: did the label map move?",
                client.name, client.map
            )),
            Ok(text) => {
                let built = keys_in(&text, client.key);
                let missing = difference(&expected, &built);
                let unknown = difference(&built, &expected);
                if !missing.is_empty() {
                    failures.push(format!(
                        "{} ({}): does not send {} label(s): {}",
                        client.name,
                        client.map,
                        missing.len(),
                        missing.join(", ")
                    ));
                }
                if !unknown.is_empty() {
                    failures.push(format!(
                        "{} ({}): sends {} label(s) the bundle ignores: {}",
                        client.name,
                        client.map,
                        unknown.len(),
                        unknown.join(", ")
                    ));
                }
            }
        }
        if !calls_hook(root, client)? {
            failures.push(format!(
                "{}: never calls window.{HOOK}: its toolbar stays English",
                client.name
            ));
        }
    }

    if failures.is_empty() {
        println!(
            "Editor labels: {} keys, sent by all {} clients.",
            expected.len(),
            CLIENTS.len()
        );
        return Ok(true);
    }

    eprintln!("Editor labels are out of sync with {BUNDLE}:\n");
    for failure in &failures {
        eprintln!("  {failure}");
    }
    eprintln!(
        "\nAdd the key to every client's label map (and a catalog `editor_*` string for it),\nor \
         remove it from the bundle's Labels interface."
    );
    Ok(false)
}

/// The fields of the bundle's `Labels` interface.
///
/// # Errors
///
/// Fails when the block is absent or has no fields: either means the shape moved, and a key list
/// that parsed to nothing is a check that cannot fail.
fn bundle_keys(text: &str) -> Result<Vec<String>, String> {
    let opener = "export interface Labels {";
    let start = text.find(opener).ok_or_else(|| {
        format!("{BUNDLE}: no `export interface Labels` block: has it been renamed?")
    })? + opener.len();
    let body = &text[start..];
    let end = body
        .find("\n}")
        .ok_or_else(|| format!("{BUNDLE}: the `Labels` block is never closed."))?;

    let mut keys: Vec<String> = Vec::new();
    for line in body[..end].lines() {
        let trimmed = line.trim_start();
        let name: String = trimmed
            .chars()
            .take_while(char::is_ascii_alphabetic)
            .collect();
        if !name.is_empty() && trimmed[name.len()..].starts_with(": string;") {
            keys.push(name);
        }
    }
    if keys.is_empty() {
        return Err(format!(
            "{BUNDLE}: `Labels` has no fields: the shape must have gone stale."
        ));
    }
    keys.sort();
    keys.dedup();
    Ok(keys)
}

/// The quoted keys a client's map builds, found by the text each language puts around one.
fn keys_in(text: &str, (before, after): (&str, &str)) -> Vec<String> {
    let mut keys: Vec<String> = Vec::new();
    for line in text.lines() {
        let mut rest = line;
        while let Some(at) = rest.find(before) {
            let candidate = &rest[at + before.len()..];
            let name: String = candidate
                .chars()
                .take_while(char::is_ascii_alphabetic)
                .collect();
            if !name.is_empty() && candidate[name.len()..].starts_with(after) {
                keys.push(name.clone());
            }
            rest = &candidate[name.len().max(1).min(candidate.len())..];
        }
    }
    keys.sort();
    keys.dedup();
    keys
}

/// Whether any source under the client's directory injects the hook.
///
/// # Errors
///
/// Propagates a git failure.
fn calls_hook(root: &Path, client: &Client) -> Result<bool, String> {
    let (directory, call) = client.call;
    let files = git::listed(root, &[directory])?;
    if files.is_empty() {
        return Ok(false);
    }
    for name in files {
        if !SOURCES.contains(&crate::prose::extension(&name)) {
            continue;
        }
        if std::fs::read_to_string(root.join(&name)).is_ok_and(|text| text.contains(call)) {
            return Ok(true);
        }
    }
    Ok(false)
}

/// The entries of `left` that `right` does not have.
fn difference(left: &[String], right: &[String]) -> Vec<String> {
    left.iter()
        .filter(|key| !right.contains(key))
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{bundle_keys, difference, keys_in};

    const LABELS: &str = "export interface Labels {\n  bold: string;\n  italic: string;\n  // a \
                          comment\n  link: string;\n}\n\nexport const DEFAULTS = {};\n";

    #[test]
    fn reads_the_interfaces_fields_and_nothing_else() {
        assert_eq!(bundle_keys(LABELS).unwrap(), ["bold", "italic", "link"]);
    }

    #[test]
    fn a_renamed_or_empty_interface_is_an_error_not_an_empty_list() {
        // Comparing an empty list to an empty list is the check that cannot fail.
        assert!(bundle_keys("export const Labels = {};\n").is_err());
        assert!(bundle_keys("export interface Labels {\n}\n").is_err());
    }

    #[test]
    fn reads_each_languages_spelling_of_the_same_map() {
        let kotlin = "        put(\"bold\", L10n.editorBold())\n        put(\"italic\", x)\n";
        assert_eq!(keys_in(kotlin, ("put(\"", "\"")), ["bold", "italic"]);

        let swift = "        \"bold\": L10n.editorBold,\n        \"link\": L10n.editorLink,\n";
        assert_eq!(keys_in(swift, ("\"", "\": L10n.")), ["bold", "link"]);

        let rust = "        \"bold\": l10n::editor_bold(),\n";
        assert_eq!(keys_in(rust, ("\"", "\": l10n::")), ["bold"]);

        let csharp = "        [\"bold\"] = L10n.EditorBold,\n";
        assert_eq!(keys_in(csharp, ("[\"", "\"] = L10n.")), ["bold"]);
    }

    #[test]
    fn a_quoted_string_that_is_not_a_key_is_not_read_as_one() {
        // The text after the key is what tells a map entry from an ordinary string.
        let swift = "        let title = \"bold\"\n";
        assert!(keys_in(swift, ("\"", "\": L10n.")).is_empty());
    }

    #[test]
    fn the_difference_is_what_the_report_names() {
        let bundle = ["bold".to_owned(), "italic".to_owned()];
        let built = ["bold".to_owned(), "strike".to_owned()];
        assert_eq!(difference(&bundle, &built), ["italic"]);
        assert_eq!(difference(&built, &bundle), ["strike"]);
    }
}
