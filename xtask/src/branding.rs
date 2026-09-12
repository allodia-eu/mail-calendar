//! Fails when a client has stopped taking its identity from the brand.
//!
//! The app's name and application id are injected at build time (`docs/branding.md`): `branding/`
//! holds them, every client's build config reads them, and the unbranded default is what a build
//! carries when nothing overrides it. Nothing about that arrangement fails loudly on its own: a
//! literal written back into a manifest builds, installs and runs, and simply cannot be re-branded
//! any more. So the committed defaults are pinned, and each client is asserted still to *derive*
//! rather than state.
//!
//! What this deliberately does not check is prose. Documentation, comments and store copy still
//! name the product; a search for the word here would fail on every file that correctly explains
//! what a branded build is.

use std::path::Path;

use crate::{git, report::Report};

/// Runs the check. `Ok(true)` means every client still derives its identity.
///
/// # Errors
///
/// Fails when `branding/default.env` or `branding/default-listing.md` is missing: the first is the
/// identity every build falls back to, the second the copy it describes itself with.
pub(crate) fn run(root: &Path) -> Result<bool, String> {
    let mut report = Report::new();

    let env = git::read(root, "branding/default.env").map_err(|_| {
        "branding/default.env is missing: it is the identity every build falls back to.".to_owned()
    })?;

    let neutral_id = env_value(&env, "MAILCAL_APP_ID").unwrap_or_default();
    let neutral_name = env_value(&env, "MAILCAL_APP_NAME").unwrap_or_default();
    if neutral_id.is_empty() {
        report.note("branding/default.env names no MAILCAL_APP_ID.");
    }
    if neutral_name.is_empty() {
        report.note("branding/default.env names no MAILCAL_APP_NAME.");
    }

    neutral_listing(root, &mut report)?;

    // The id is a URI scheme on Android, where an intent filter's scheme is matched
    // case-sensitively and an uppercase one silently never matches. It is also a Flatpak app id,
    // which needs three components.
    if !is_reverse_dns(&neutral_id) {
        report.note(format!(
            "MAILCAL_APP_ID '{neutral_id}' must be three or more lowercase reverse-DNS components."
        ));
    }

    clients_derive(root, &mut report)?;
    packaged_manifests(root, &neutral_id, &neutral_name, &mut report)?;
    art_and_copy(root, &mut report)?;

    if report.failed() {
        report.emit();
        eprintln!("ERROR: the app's identity is no longer injected everywhere (docs/branding.md).");
        return Ok(false);
    }

    println!("OK: every client takes its name and application id from branding/ ({neutral_id}).");
    Ok(true)
}

/// The store copy an unbranded build describes itself with.
///
/// Its absence is the one failure nothing else reports: a Flatpak build resolves it, finds nothing,
/// and the app has no entry in a software centre at all. On Linux only, at packaging time, in a
/// checkout with no brand file, which is exactly the checkout a fork has.
fn neutral_listing(root: &Path, report: &mut Report) -> Result<(), String> {
    let listing = git::read(root, "branding/default-listing.md").map_err(|_| {
        "branding/default-listing.md is missing: it is the store copy an unbranded build describes \
         itself with, and the Linux metainfo cannot be generated without it."
            .to_owned()
    })?;
    for heading in [
        "Google Play — Short description",
        "Shared description — English",
    ] {
        if !listing.contains(heading) {
            report.note(format!(
                "branding/default-listing.md has no '{heading}' section; flatpak_metadata.py needs \
                 both."
            ));
        }
    }
    Ok(())
}

/// The build configs that must read the brand rather than state it.
fn clients_derive(root: &Path, report: &mut Report) -> Result<(), String> {
    let gradle_path = "clients/android/app/build.gradle.kts";
    let gradle = git::read(root, gradle_path)?;
    require(
        &gradle,
        r#"applicationId = brandValue("MAILCAL_APP_ID")"#,
        &format!("{gradle_path} no longer takes applicationId from the brand."),
        report,
    );
    require(
        &gradle,
        r#"manifestPlaceholders["appName"] = brandValue("MAILCAL_APP_NAME")"#,
        &format!("{gradle_path} no longer takes the launcher label from the brand."),
        report,
    );

    let manifest_path = "clients/android/app/src/main/AndroidManifest.xml";
    let manifest = git::read(root, manifest_path)?;
    require(
        &manifest,
        r#"android:label="${appName}""#,
        &format!(
            "{manifest_path} has a literal android:label: it must come from the appName \
             placeholder."
        ),
        report,
    );
    // The OAuth redirects are the app id; a literal here is a filter that stops matching the moment
    // the app is re-branded, which is a sign-in that dies on delivery with nothing logged.
    require(
        &manifest,
        r#"android:host="${applicationId}""#,
        &format!("{manifest_path} has a literal Microsoft redirect host."),
        report,
    );
    require(
        &manifest,
        r#"android:scheme="${applicationId}""#,
        &format!("{manifest_path} has a literal JMAP redirect scheme."),
        report,
    );

    let project_path = "clients/apple/project.yml";
    let project = git::read(root, project_path)?;
    require(
        &project,
        "PRODUCT_BUNDLE_IDENTIFIER: ${MAILCAL_APP_ID}",
        &format!("{project_path} no longer takes the bundle id from the environment."),
        report,
    );
    require(
        &project,
        "CFBundleDisplayName: ${MAILCAL_APP_NAME}",
        &format!("{project_path} no longer takes the display name from the environment."),
        report,
    );

    for ents in [
        "clients/apple/App/AllodiaMail.entitlements",
        "clients/apple/App/AllodiaMail.appstore.entitlements",
    ] {
        require(
            &git::read(root, ents)?,
            "<string>$(AppIdentifierPrefix)$(PRODUCT_BUNDLE_IDENTIFIER)</string>",
            &format!("{ents} names a keychain group that does not follow the bundle id."),
            report,
        );
    }
    Ok(())
}

/// The MSIX and Flatpak manifests, which are committed carrying the neutral identity and rewritten
/// by their packaging scripts, so what is checked is that the committed copy is still neutral.
fn packaged_manifests(
    root: &Path,
    neutral_id: &str,
    neutral_name: &str,
    report: &mut Report,
) -> Result<(), String> {
    let appx_path = "clients/windows/Mailcal/Package.appxmanifest";
    let appx = git::read(root, appx_path)?;
    require(
        &appx,
        &format!(r#"<uap:Protocol Name="{neutral_id}">"#),
        &format!(
            "{appx_path} does not declare the neutral protocol '{neutral_id}': package.ps1 \
             rewrites that one."
        ),
        report,
    );
    require(
        &appx,
        &format!("<DisplayName>{neutral_name}</DisplayName>"),
        &format!("{appx_path} does not carry the neutral display name '{neutral_name}'."),
        report,
    );

    let flatpak_path = format!("clients/linux/flatpak/{neutral_id}.yml");
    match git::read(root, &flatpak_path) {
        Err(_) => report.note(format!(
            "the Flatpak manifest is not named for the neutral id ({flatpak_path} is missing)."
        )),
        Ok(flatpak) => {
            if !flatpak
                .lines()
                .any(|l| l.trim_end() == format!("app-id: {neutral_id}"))
            {
                report.note(format!(
                    "{flatpak_path} does not carry the neutral app id: package.sh rewrites that \
                     one."
                ));
            }
        }
    }
    Ok(())
}

/// The art, which is a brand slot, and the one string the app says aloud.
fn art_and_copy(root: &Path, report: &mut Report) -> Result<(), String> {
    // Every launcher icon is cut from one source, resolved the same way the values are. The failure
    // this catches is a generator quietly pinned back to a path: it keeps working on the branded
    // art, and only an unbranded build notices, by shipping somebody else's icon.
    for art in ["branding/default-icon.png", "branding/default-welcome.png"] {
        if !root.join(art).is_file() {
            let what = if art.ends_with("icon.png") {
                "icon"
            } else {
                "illustration"
            };
            report.note(format!("{art} is missing: the neutral {what}."));
        }
    }
    require(
        &git::read(root, "scripts/dev/brand-welcome.sh")?,
        "brand_welcome_source",
        "scripts/dev/brand-welcome.sh no longer resolves its source through the brand.",
        report,
    );
    for generator in [
        "clients/apple/Scripts/generate-appicon.sh",
        "clients/android/generate-icons.sh",
        "clients/linux/flatpak/generate-icons.sh",
    ] {
        require(
            &git::read(root, generator)?,
            "brand_icon_source",
            &format!("{generator} no longer resolves its source through the brand."),
            report,
        );
    }
    let assets = "clients/windows/Mailcal/Images/generate-assets.ps1";
    if !git::read(root, assets)?
        .lines()
        .any(|l| l.contains("brand.py") && l.contains("--icon-source"))
    {
        report.note(format!(
            "{assets} no longer resolves its source through the brand."
        ));
    }

    // The art is a brand slot, so its label has to describe the slot; and the product name is
    // substituted into the catalog at codegen time, so a locale that hardcoded it would be one
    // language showing a different app's name.
    for catalog in git::ls_files(root, &["messages/*.json"])? {
        let text = git::read(root, &catalog)?;
        require(
            &text,
            r#""a11y_welcome_art""#,
            &format!("{catalog} is missing a11y_welcome_art."),
            report,
        );
        require(
            &text,
            r#""app_title": "{app_name}""#,
            &format!("{catalog} does not leave app_title as the {{app_name}} placeholder."),
            report,
        );
    }
    Ok(())
}

/// Notes `why` unless `haystack` carries `needle`.
fn require(haystack: &str, needle: &str, why: &str, report: &mut Report) {
    if !haystack.contains(needle) {
        report.note(why);
    }
}

/// The last value assigned to `key`, with optional surrounding quotes removed.
///
/// Read directly rather than through the brand helper: this checks what is *committed*, and a
/// checkout carrying a brand (or an exported variable) would otherwise answer with that instead.
fn env_value(env: &str, key: &str) -> Option<String> {
    let prefix = format!("{key}=");
    env.lines()
        .filter_map(|line| line.trim_end().strip_prefix(&prefix))
        .map(|v| v.trim_matches('"').to_owned())
        .next_back()
}

/// Three or more lowercase reverse-DNS components.
fn is_reverse_dns(id: &str) -> bool {
    let parts: Vec<&str> = id.split('.').collect();
    parts.len() >= 3
        && parts.iter().all(|p| {
            !p.is_empty()
                && p.chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        })
}

#[cfg(test)]
mod tests {
    use super::{env_value, is_reverse_dns};

    #[test]
    fn reads_a_quoted_or_bare_value() {
        assert_eq!(
            env_value("MAILCAL_APP_ID=\"eu.allodia.mail\"\n", "MAILCAL_APP_ID").as_deref(),
            Some("eu.allodia.mail")
        );
        assert_eq!(
            env_value("MAILCAL_APP_ID=eu.allodia.mail\n", "MAILCAL_APP_ID").as_deref(),
            Some("eu.allodia.mail")
        );
    }

    #[test]
    fn the_last_assignment_wins() {
        let env = "MAILCAL_APP_ID=first.one.here\nMAILCAL_APP_ID=second.one.here\n";
        assert_eq!(
            env_value(env, "MAILCAL_APP_ID").as_deref(),
            Some("second.one.here")
        );
    }

    #[test]
    fn an_uppercase_id_is_rejected() {
        // An Android intent filter matches a scheme case-sensitively, so this never fires.
        assert!(!is_reverse_dns("eu.Allodia.mail"));
        assert!(is_reverse_dns("eu.allodia.mail"));
    }

    #[test]
    fn two_components_are_not_enough_for_a_flatpak_id() {
        assert!(!is_reverse_dns("allodia.mail"));
    }
}
