//! Guards how the Linux client hands a URI or a file to the rest of the desktop.
//!
//! `g_app_info_launch_default_for_uri` and its siblings resolve against the desktop's application
//! database. A Flatpak has no such database, so GIO falls back through GVFS onto the session bus,
//! and the call is synchronous, on the GTK main thread. Measured on the shape that ships: the whole
//! app froze, with no repaint and no Cancel button left to press, on every attempt to open the
//! browser for a sign-in.
//!
//! It fails in the worst possible way. A developer running against the distribution's GTK, outside
//! the sandbox, sees it work perfectly, because there the application database is right there. Only
//! the packaged build wedges, which is the one nobody runs while iterating, and no test can see it,
//! because reproducing it needs a portal and a sandbox.

use std::path::Path;

use crate::git::{self, Kind};

/// The call names that reach the host application database.
///
/// `show_uri` is included for the same reason: it is GTK's older, pre-portal spelling, kept working
/// but not portal-aware everywhere.
const BANNED: &[&str] = &[
    "AppInfo::launch_default_for_uri",
    "AppInfo::launch_uris",
    "AppInfo::launch(",
    "gtk::show_uri",
    "show_uri_full",
];

/// Runs the check. `Ok(true)` means every hand-off goes through a portal launcher.
///
/// # Errors
///
/// Propagates a git failure. That matters more here than elsewhere: an error that read as "no
/// matches" is how this very check once passed while the call it bans sat in the tree.
pub(crate) fn run(root: &Path) -> Result<bool, String> {
    let mut failed = false;

    for call in BANNED {
        let hits = git::grep(root, Kind::Fixed, call, &["clients/linux/**/*.rs"])?;
        if !hits.is_empty() {
            println!(
                "ERROR: {call} reaches the desktop application database, which a Flatpak does not \
                 have."
            );
            for line in &hits.lines {
                println!("    {line}");
            }
            failed = true;
        }
    }

    if failed {
        eprintln!(
            "
Use the portal-shaped launchers instead. They are asynchronous, so they cannot freeze the window
they were called from, and they ask the OpenURI portal rather than a database the sandbox lacks:

    a URI   gtk::UriLauncher::new(uri).launch(parent, cancellable, move |result| {{ … }})
    a file  gtk::FileLauncher::new(Some(&file)).launch(parent, cancellable, move |result| {{ … }})

The failure arrives in the callback rather than as a return value, so route it through an AppInput
the way clients/linux/src/ui/operations.rs does for an attachment."
        );
        return Ok(false);
    }

    println!(
        "OK: the Linux client hands every URI and file to the desktop through the portal launchers."
    );
    Ok(true)
}
