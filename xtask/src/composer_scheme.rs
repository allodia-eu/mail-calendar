//! Fails when a host of the shared editor does not tell it the app's light or dark.
//!
//! The bundle draws its toolbar and panels in the scheme its host names through
//! `window.setComposerScheme` (`clients/composer/src/scheme.ts`), and falls back to the web view's
//! `prefers-color-scheme` until one does. That fallback is right on some engines and wrong on
//! others: WebView2, Android's WebView and WebKitGTK answer the system, so a Light or Dark chosen
//! in the app is missed. A host that stops naming the scheme still draws a toolbar, in the system's
//! colours inside the app's, and nothing throws, logs or fails a build.
//!
//! Each editor surface (the composer, the signature editor) is checked on every client, at the
//! place that names the scheme on load and on change, plus the script the client builds.

use std::path::Path;

use crate::git;

/// One thing a client has to say, and where.
struct Site {
    client: &'static str,
    file: &'static str,
    needle: &'static str,
    /// What is lost when it goes, for the report.
    what: &'static str,
}

const SITES: &[Site] = &[
    Site {
        client: "bundle",
        file: "clients/composer/src/main.ts",
        needle: "window.setComposerScheme =",
        what: "the hook every host calls",
    },
    Site {
        client: "android",
        file: "clients/android/app/src/main/java/eu/allodia/mailcal/EditorWebView.kt",
        needle: "window.setComposerScheme(",
        what: "the script both editors send",
    },
    Site {
        client: "android",
        file: "clients/android/app/src/main/java/eu/allodia/mailcal/ComposerEditorHost.kt",
        needle: "composerSchemeScript(",
        what: "the composer's scheme",
    },
    Site {
        client: "android",
        file: "clients/android/app/src/main/java/eu/allodia/mailcal/SignatureEditorDialog.kt",
        needle: "composerSchemeScript(",
        what: "the signature editor's scheme",
    },
    Site {
        client: "apple",
        file: "clients/apple/Packages/MailcalKit/Sources/MailcalUI/EditorHost.swift",
        needle: "window.setComposerScheme(",
        what: "the script both editors send",
    },
    Site {
        client: "apple",
        file: "clients/apple/Packages/MailcalKit/Sources/MailcalUI/RichComposerView.swift",
        needle: "host.setDark(",
        what: "the composer's scheme",
    },
    Site {
        client: "apple",
        file: "clients/apple/Packages/MailcalKit/Sources/MailcalUI/SignatureEditorView.swift",
        needle: "host.setDark(",
        what: "the signature editor's scheme",
    },
    Site {
        client: "linux",
        file: "clients/linux/src/ui/composer_scheme.rs",
        needle: "window.setComposerScheme(",
        what: "the script both editors send",
    },
    Site {
        client: "linux",
        file: "clients/linux/src/ui/composer.rs",
        needle: "composer_scheme::current_script()",
        what: "the composer's scheme at load",
    },
    Site {
        client: "linux",
        file: "clients/linux/src/ui/composer.rs",
        needle: "composer_scheme::follow(",
        what: "the composer's scheme on change",
    },
    Site {
        client: "linux",
        file: "clients/linux/src/ui/settings/signature_editor.rs",
        needle: "composer_scheme::current_script()",
        what: "the signature editor's scheme at load",
    },
    Site {
        client: "linux",
        file: "clients/linux/src/ui/settings/signature_editor.rs",
        needle: "composer_scheme::follow(",
        what: "the signature editor's scheme on change",
    },
    // One host serves both editors on Windows.
    Site {
        client: "windows",
        file: "clients/windows/Mailcal/Services/EditorWebView.cs",
        needle: "window.setComposerScheme(",
        what: "the script both editors send",
    },
    Site {
        client: "windows",
        file: "clients/windows/Mailcal/Services/EditorWebView.cs",
        needle: "ActualThemeChanged += (_, _) => _ = SyncSchemeAsync();",
        what: "both editors' scheme on change",
    },
    Site {
        client: "windows",
        file: "clients/windows/Mailcal/Services/EditorWebView.cs",
        needle: "await SyncSchemeAsync();",
        what: "both editors' scheme at load",
    },
];

/// Runs the check. `Ok(true)` means every host names the app's scheme.
///
/// # Errors
///
/// Fails when git cannot list the tree. A file it does not list is reported as a failure instead,
/// since a moved host is exactly the change this exists to notice.
pub(crate) fn run(root: &Path) -> Result<bool, String> {
    let files: Vec<&str> = SITES.iter().map(|site| site.file).collect();
    let listed = git::listed(root, &files)?;
    let failures = failures(SITES, |file| {
        listed
            .iter()
            .any(|name| name == file)
            .then(|| git::read(root, file).ok())
            .flatten()
    });
    if failures.is_empty() {
        println!(
            "Editor scheme: named at {} sites across the bundle and 4 clients.",
            SITES.len()
        );
        return Ok(true);
    }
    eprintln!("A host of the shared editor no longer names the app's light or dark:\n");
    for failure in &failures {
        eprintln!("  {failure}");
    }
    eprintln!(
        "\nWithout it the toolbar follows the web view's own scheme, which on some platforms is \
         the system's.\nRestore the call, or move this check with it."
    );
    Ok(false)
}

fn failures(sites: &[Site], read: impl Fn(&str) -> Option<String>) -> Vec<String> {
    sites
        .iter()
        .filter_map(|site| match read(site.file) {
            None => Some(format!(
                "{}: {} is missing: did the host move? ({})",
                site.client, site.file, site.what
            )),
            Some(text) if !text.contains(site.needle) => Some(format!(
                "{}: {} no longer has `{}`: {} is lost",
                site.client, site.file, site.needle, site.what
            )),
            Some(_) => None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{Site, failures};

    const SITE: Site = Site {
        client: "apple",
        file: "a.swift",
        needle: "host.setDark(",
        what: "the composer's scheme",
    };

    #[test]
    fn a_site_that_still_calls_passes() {
        let found = failures(&[SITE], |_| Some("editor.host.setDark(true)".to_owned()));
        assert_eq!(found, Vec::<String>::new());
    }

    #[test]
    fn a_removed_call_and_a_moved_file_both_fail() {
        assert_eq!(failures(&[SITE], |_| Some(String::new())).len(), 1);
        assert_eq!(failures(&[SITE], |_| None).len(), 1);
    }
}
