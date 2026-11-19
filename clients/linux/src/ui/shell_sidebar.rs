//! The folder pane's own chrome: the header and destination switcher around the account tree,
//! and the width the pane reopens at. Split from `shell` to keep each file under the 500-line
//! limit.

use adw::prelude::*;

use super::{PrimaryView, destinations::DestinationBar, folder_pane};
use crate::{l10n, preferences};

/// The folder pane's column: its header, and what scrolls under it.
pub(super) struct Sidebar {
    pub(super) pane: adw::ToolbarView,
    pub(super) heading: adw::HeaderBar,
    title: adw::WindowTitle,
    content: gtk::Stack,
}

impl Sidebar {
    /// The calendar pane while the calendar is on screen, the account tree otherwise: mail and
    /// contacts keep the folders (`docs/calendar-pane.md`, rule 1).
    pub(super) fn show(&self, primary: PrimaryView) {
        let calendar = primary == PrimaryView::Calendar;
        self.content
            .set_visible_child_name(if calendar { "calendar" } else { "folders" });
        self.title.set_title(if calendar {
            l10n::nav_calendar()
        } else {
            l10n::sidebar_accounts()
        });
    }

    #[cfg(test)]
    pub(super) fn showing(&self) -> Option<String> {
        self.content
            .visible_child_name()
            .map(|name| name.to_string())
    }
}

/// Assembles the folder pane: every account's tree, or the calendar pane beside the calendar,
/// scrolling under a header, with the destination switcher pinned beneath it. Answers the header
/// too, which only the calendar and contacts show: over mail, the window's band already stands
/// above the pane (`docs/command-bar.md`, rule 2).
///
/// The switcher is a **bottom bar** of the toolbar view rather than a row inside the scrolled
/// tree, which is what makes it survive a long folder list: the bar's height is reserved before
/// the accounts get theirs, so an account with fifty folders scrolls for as long as it likes and
/// the calendar, contacts and settings stay where the user last saw them.
///
/// The header carries no way to add an account: a later one is added from Settings → Accounts
/// alone (`docs/settings.md`).
pub(super) fn sidebar_pane(
    accounts: &gtk::ScrolledWindow,
    calendar: &gtk::Box,
    destinations: &DestinationBar,
) -> Sidebar {
    let pane = adw::ToolbarView::new();
    // The sidebar's own surface, which the mail window's header band shares
    // (`docs/command-bar.md`, rule 2).
    pane.add_css_class("sidebar-pane");
    let header = adw::HeaderBar::new();
    header.set_show_start_title_buttons(false);
    header.set_show_end_title_buttons(false);
    let title = adw::WindowTitle::new(l10n::sidebar_accounts(), "");
    header.set_title_widget(Some(&title));
    pane.add_top_bar(&header);
    let content = gtk::Stack::new();
    content.add_named(accounts, Some("folders"));
    content.add_named(calendar, Some("calendar"));
    pane.set_content(Some(&content));
    pane.add_bottom_bar(destinations.widget());
    Sidebar {
        pane,
        heading: header,
        title,
        content,
    }
}

/// Opens the folder pane at the width the user last left it, and keeps it there.
///
/// The width is the host's to remember (the core has no notion of a pane), and it is remembered
/// because an account address is as long as it is: at a fixed width the row that gets clipped
/// mid-domain is precisely the one with several accounts to tell apart. The clamp is applied
/// against the window's own width, so a pane dragged wide on a large monitor cannot open on a
/// small one with no mail beside it.
pub(super) fn restore_pane_width(pane: &gtk::Paned, root: &adw::ApplicationWindow) {
    let stored = preferences::global()
        .folder_pane_width()
        .unwrap_or(folder_pane::width::DEFAULT);
    pane.set_position(folder_pane::width::clamp(stored, root.default_width()));
    pane.connect_position_notify(|pane| {
        preferences::global().set_folder_pane_width(pane.position());
    });
}
