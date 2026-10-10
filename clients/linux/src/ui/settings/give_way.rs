//! When the Settings window steps aside for the main window's composer, and when it stays.

use super::SettingsWindow;

/// The main window's composer, as one render finds it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PaneComposer {
    Absent,
    /// This render puts it in the pane.
    Arriving,
    /// An earlier render put it there.
    Shown,
}

impl PaneComposer {
    pub(crate) const fn of(open: bool, already_shown: bool) -> Self {
        match (open, already_shown) {
            (false, _) => Self::Absent,
            (true, false) => Self::Arriving,
            (true, true) => Self::Shown,
        }
    }
}

impl SettingsWindow {
    /// Takes the window down for what has to be in front of the person instead: a composer
    /// arriving in the pane, or a navigation waiting for the pane's composer to be left.
    ///
    /// A composer already in the pane leaves it open. Settings is a window of its own, so opening
    /// it does not leave the composer, which stays where it is with its draft (`docs/drafts.md`),
    /// as it does under Settings on macOS and Windows.
    pub(crate) fn give_way(&mut self, composer: PaneComposer, leaving_composer: bool) {
        if composer == PaneComposer::Arriving || leaving_composer {
            self.close();
        }
    }
}
