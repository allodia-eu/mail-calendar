//! The folder pane's own chrome: the header and destination switcher around the account tree,
//! and the width the pane reopens at. Split from `shell` to keep each file under the 500-line
//! limit.

use adw::prelude::*;
use gtk::accessible::Property as AccessibleProperty;

use super::{AppInput, destinations::DestinationBar, folder_pane};
use crate::{l10n, preferences};

/// Assembles the folder pane: every account's tree scrolling under a header, with the destination
/// switcher pinned beneath it.
///
/// The switcher is a **bottom bar** of the toolbar view rather than a row inside the scrolled
/// tree, which is what makes it survive a long folder list: the bar's height is reserved before
/// the accounts get theirs, so an account with fifty folders scrolls for as long as it likes and
/// the calendar, contacts and settings stay where the user last saw them.
pub(super) fn sidebar_pane(
    sender: &relm4::Sender<AppInput>,
    accounts: &gtk::ScrolledWindow,
    destinations: &DestinationBar,
) -> adw::ToolbarView {
    let pane = adw::ToolbarView::new();
    let header = adw::HeaderBar::new();
    header.set_show_start_title_buttons(false);
    header.set_show_end_title_buttons(false);
    header.set_title_widget(Some(&adw::WindowTitle::new(l10n::sidebar_accounts(), "")));
    let add_account = gtk::Button::from_icon_name("list-add-symbolic");
    add_account.set_tooltip_text(Some(l10n::action_add_account()));
    add_account.update_property(&[AccessibleProperty::Label(l10n::action_add_account())]);
    let input = sender.clone();
    add_account.connect_clicked(move |_| input.emit(AppInput::OpenAccountSetup));
    header.pack_start(&add_account);
    pane.add_top_bar(&header);
    pane.set_content(Some(accounts));
    pane.add_bottom_bar(destinations.widget());
    pane
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
