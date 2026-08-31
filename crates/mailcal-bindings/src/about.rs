//! What a client's Settings → About shows, decided here so every client says the same thing.
//!
//! The version, the support address and the list of open-source work the app is built on are
//! product facts, not per-client copy: a support answer that names a version has to name the same
//! version on every platform, and an attribution that appears on one client and not another is a
//! notice we have not really given. Only the surrounding labels are localised, in each client's
//! own catalog.

/// The support address printed in Settings → About on every client.
///
/// The Discourse the help pages already send people to, not the marketing page.
const SUPPORT_URL: &str = "https://support.allodia.eu";

/// One piece of third-party work the app is built on, and the licence it is used under.
#[derive(uniffi::Record)]
pub struct Attribution {
    /// The component's name, as its authors write it.
    pub name: String,
    /// Its licence, by SPDX identifier.
    pub license: String,
}

/// The About surface's content: the release the user is running, where to ask for help, and what
/// the app is built on.
#[derive(uniffi::Record)]
pub struct AboutInfo {
    /// The app version, the one [`/VERSION`](../../../VERSION) holds; `check-version-sync.sh`
    /// keeps the crate version equal to it, so this cannot drift from what a release announces.
    pub version: String,
    /// Where to ask for help.
    pub support_url: String,
    /// Everything the app is built on that is somebody else's work.
    pub attributions: Vec<Attribution>,
}

/// The About content for `platform`, which names the UI toolkit that client links.
///
/// The shared entries are the ones every client ships: the Rust core and its crates. Each client
/// passes its own [`AboutPlatform`] so the toolkit it actually links is named too: attributing
/// GTK on an iPhone would be a false notice, and omitting it on Linux an absent one.
#[uniffi::export]
#[must_use]
pub fn about_info(platform: AboutPlatform) -> AboutInfo {
    let mut attributions = vec![
        attribution("Rust", "MIT OR Apache-2.0"),
        // The crate closure is overwhelmingly MIT/Apache-2.0, with MPL-2.0, ISC and Unicode-3.0
        // in it; naming those licences is the notice, and naming 300-odd crates is not something
        // a person reads. A client that wants the full list can print `cargo license`.
        attribution(
            "Open-source Rust crates",
            "MIT, Apache-2.0, MPL-2.0, ISC, Unicode-3.0",
        ),
    ];
    attributions.extend(platform.attributions());
    AboutInfo {
        version: env!("CARGO_PKG_VERSION").to_owned(),
        support_url: SUPPORT_URL.to_owned(),
        attributions,
    }
}

/// Which client is asking; it decides which UI toolkit About names.
#[derive(uniffi::Enum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum AboutPlatform {
    /// macOS, iOS and iPadOS: the frameworks are the operating system's own, so there is nothing
    /// of anybody else's to attribute.
    Apple,
    /// Windows: the app links the Windows App SDK and WinUI.
    Windows,
    /// Android: the app links AndroidX and Jetpack Compose.
    Android,
    /// Linux: the app links GTK, libadwaita and WebKitGTK.
    Linux,
}

impl AboutPlatform {
    fn attributions(self) -> Vec<Attribution> {
        match self {
            Self::Apple => Vec::new(),
            Self::Windows => vec![attribution("Windows App SDK and WinUI", "MIT")],
            Self::Android => vec![attribution("AndroidX and Jetpack Compose", "Apache-2.0")],
            Self::Linux => vec![
                attribution("GTK and libadwaita", "LGPL-2.1-or-later"),
                attribution("WebKitGTK", "LGPL-2.1-or-later AND BSD-2-Clause"),
            ],
        }
    }
}

fn attribution(name: &str, license: &str) -> Attribution {
    Attribution {
        name: name.to_owned(),
        license: license.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::{AboutPlatform, about_info};

    #[test]
    fn about_reports_the_release_the_version_file_holds() {
        let about = about_info(AboutPlatform::Linux);
        // `check-version-sync.sh` gates the other direction; this catches a client that reads the
        // version from somewhere else entirely.
        assert_eq!(about.version, env!("CARGO_PKG_VERSION"));
        assert!(
            about.version.split('.').count() == 3
                && about
                    .version
                    .split('.')
                    .all(|part| part.parse::<u32>().is_ok()),
            "a user reads this: {}",
            about.version
        );
        assert_eq!(about.support_url, "https://support.allodia.eu");
    }

    #[test]
    fn every_client_attributes_the_toolkit_it_actually_links() {
        for platform in [
            AboutPlatform::Apple,
            AboutPlatform::Windows,
            AboutPlatform::Android,
            AboutPlatform::Linux,
        ] {
            let about = about_info(platform);
            assert!(
                about.attributions.iter().all(|item| {
                    !item.name.trim().is_empty() && !item.license.trim().is_empty()
                }),
                "an attribution with no licence names nothing"
            );
            let names: Vec<&str> = about.attributions.iter().map(|a| a.name.as_str()).collect();
            assert!(names.contains(&"Rust"), "every client ships the core");
            let toolkit = names.iter().any(|name| name.contains("GTK"));
            assert_eq!(
                toolkit,
                platform == AboutPlatform::Linux,
                "only the client that links GTK may say so: {platform:?}"
            );
        }
    }
}
