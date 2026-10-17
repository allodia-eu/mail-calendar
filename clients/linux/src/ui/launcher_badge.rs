//! The unread count on the app's icon in a dock (`docs/app-badge.md`): the unified Inbox unread
//! the pane's All Accounts › Inbox row shows, and nothing at zero.
//!
//! Linux has no standard for this. What docks read is Unity's `LauncherEntry`, a broadcast
//! `Update` signal naming the app by its desktop-file id: Ubuntu Dock, KDE Plasma's task manager
//! and Plank draw it, and GNOME Shell without a dock extension draws nothing. Neither package
//! needs a permission for it. The Flatpak's bus proxy passes a signal addressed to nobody, and
//! the snap's `desktop` plug, which the GNOME extension adds, allows exactly this one.

use std::collections::HashMap;

use gtk::{gio, glib, prelude::*};

/// Where the signal is emitted from. Docks match on the interface and the app URI and ignore the
/// path; this one has the shape snapd's `unity7` interface allows, which libunity also uses.
const OBJECT_PATH: &str = "/com/canonical/unity/launcherentry/1";
const INTERFACE: &str = "com.canonical.Unity.LauncherEntry";
const MEMBER: &str = "Update";

/// What this process last put on the icon, so a snapshot that leaves the count alone sends
/// nothing.
#[derive(Default)]
pub(super) struct LauncherBadge {
    shown: Option<u32>,
}

impl LauncherBadge {
    pub(super) fn show(&mut self, unread: u32) {
        if self.shown == Some(unread) {
            return;
        }
        if emit(unread) {
            self.shown = Some(unread);
        }
    }
}

/// Takes the count off the icon as the app quits, since nothing updates it once the app is
/// gone.
pub(crate) fn clear() {
    if emit(0)
        && let Some(connection) = bus()
        && let Err(error) = connection.flush_sync(None::<&gio::Cancellable>)
    {
        log::warn!("app badge: could not clear ({error})");
    }
}

/// Sends the count, and says whether it went. Not before the application is on the session bus.
fn emit(unread: u32) -> bool {
    let Some(connection) = bus() else {
        return false;
    };
    let update = update(&desktop_id(), unread);
    match connection.emit_signal(None, OBJECT_PATH, INTERFACE, MEMBER, Some(&update)) {
        Ok(()) => true,
        Err(error) => {
            log::warn!("app badge: could not update ({error})");
            false
        }
    }
}

fn bus() -> Option<gio::DBusConnection> {
    gio::Application::default()?.dbus_connection()
}

/// The `Update` signal's body, `(sa{sv})`: the app's URI, then the count and whether to show it.
fn update(desktop_id: &str, unread: u32) -> glib::Variant {
    let properties = HashMap::from([
        ("count".to_owned(), i64::from(unread).to_variant()),
        ("count-visible".to_owned(), (unread > 0).to_variant()),
    ]);
    (format!("application://{desktop_id}"), properties).to_variant()
}

/// The desktop-file id a dock knows this app by.
fn desktop_id() -> String {
    desktop_id_for(
        std::env::var("SNAP_INSTANCE_NAME").ok().as_deref(),
        std::env::var("SNAP_NAME").ok().as_deref(),
    )
}

/// The application id, except in a snap: snapd installs the desktop file as
/// `<instance>_<app>.desktop`, and the snap's one app is named after the snap
/// (`snapcraft.yaml.in`).
fn desktop_id_for(snap_instance: Option<&str>, snap_name: Option<&str>) -> String {
    match (snap_instance, snap_name) {
        (Some(instance), Some(app)) => format!("{instance}_{app}.desktop"),
        _ => format!("{}.desktop", crate::l10n::APP_ID),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use gtk::glib::{self, prelude::FromVariant};

    use super::{desktop_id_for, update};

    fn read(update: &glib::Variant) -> (String, i64, bool) {
        let (uri, properties) =
            <(String, HashMap<String, glib::Variant>)>::from_variant(update).expect("(sa{sv})");
        let count = properties["count"].get::<i64>().expect("count is an x");
        let visible = properties["count-visible"]
            .get::<bool>()
            .expect("count-visible is a b");
        (uri, count, visible)
    }

    #[test]
    fn the_signal_carries_the_count_under_the_apps_uri() {
        let signal = update("org.mailcal.client.desktop", 548);
        assert_eq!(signal.type_().as_str(), "(sa{sv})");
        assert_eq!(
            read(&signal),
            (
                "application://org.mailcal.client.desktop".to_owned(),
                548,
                true
            )
        );
    }

    #[test]
    fn zero_unread_hides_the_count() {
        // Nothing unread and a provider that reports no count are both zero, and neither has a
        // number worth drawing (`docs/folder-pane.md`, rule 6).
        let (_, count, visible) = read(&update("org.mailcal.client.desktop", 0));
        assert_eq!((count, visible), (0, false));
    }

    #[test]
    fn a_snap_is_known_by_its_instance_and_app() {
        // A dock matches the URI against the desktop file snapd installed, which is never named
        // after the application id.
        assert_eq!(
            desktop_id_for(Some("mailcal_beta"), Some("mailcal")),
            "mailcal_beta_mailcal.desktop"
        );
        assert_eq!(
            desktop_id_for(None, None),
            format!("{}.desktop", crate::l10n::APP_ID)
        );
    }
}
