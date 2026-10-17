//! Where the client keeps what it stores on this device.

use std::{ffi::OsString, path::PathBuf};

/// The directory the mail store, the diagnostic log and the MCP endpoint live under.
///
/// In a snap, the XDG data directory is inside the revision's own directory, which snapd copies
/// whole at every refresh so that a revert finds it as it was. A mail store is far too large to
/// copy on every update, so a snap keeps it in the directory all of its revisions share instead.
/// Elsewhere this is GLib's data directory, which Flatpak already points into the app's own.
pub(crate) fn data_root() -> PathBuf {
    root_for(std::env::var_os("SNAP_USER_COMMON")).join("mailcal")
}

fn root_for(snap_user_common: Option<OsString>) -> PathBuf {
    snap_user_common
        .filter(|directory| !directory.is_empty())
        .map_or_else(gtk::glib::user_data_dir, |directory| {
            PathBuf::from(directory).join(".local/share")
        })
}

#[cfg(test)]
mod tests {
    use std::{ffi::OsString, path::Path};

    use super::root_for;

    #[test]
    fn a_snap_keeps_its_data_where_every_revision_reads_it() {
        assert_eq!(
            root_for(Some(OsString::from("/home/a/snap/mailcal/common"))),
            Path::new("/home/a/snap/mailcal/common/.local/share")
        );
    }

    #[test]
    fn outside_a_snap_the_data_directory_is_glibs() {
        assert_eq!(root_for(None), gtk::glib::user_data_dir());
        assert_eq!(root_for(Some(OsString::new())), gtk::glib::user_data_dir());
    }
}
