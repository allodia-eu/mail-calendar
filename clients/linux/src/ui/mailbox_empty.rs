//! Why the mail list has no rows, drawn in the list's own placeholder.
//!
//! The badge beside the folder counts what the server holds over all time, while the list can
//! only show what sync depth kept (`docs/folder-pane.md`, rule 5). A folder of older mail then
//! badges its unread above nothing at all, and without this nothing on screen reconciles the two
//! numbers.
//!
//! A `GtkListBox` placeholder is shown exactly when the box holds no rows, so the rendering path
//! beside this one never has to ask whether it is drawing an empty list: it renders rows, and GTK
//! decides which of the two the user sees.

use adw::prelude::*;
use mailcal_bindings::EmptyReason;

use super::AppInput;
use crate::l10n;

/// The icon for a folder whose mail is simply not here yet, and for one the depth is keeping out.
const EMPTY_ICON: &str = "mail-mark-unread-symbolic";
const WINDOWED_ICON: &str = "document-open-recent-symbolic";

/// Puts the reason for an empty list into `list`'s placeholder, or takes the placeholder away
/// when the core gave no reason (a list with rows, or a search, which says how far it looked
/// through the horizon strip instead).
pub(super) fn render(
    list: &gtk::ListBox,
    reason: Option<&EmptyReason>,
    sender: &relm4::Sender<AppInput>,
) {
    let Some(reason) = reason else {
        list.set_placeholder(None::<&gtk::Widget>);
        return;
    };
    let page = adw::StatusPage::new();
    match reason {
        EmptyReason::NoMail => {
            // Nothing was held back, so there is no setting to offer: a button here would
            // promise mail that widening cannot find.
            page.set_icon_name(Some(EMPTY_ICON));
            page.set_title(l10n::mailbox_empty_no_mail());
        }
        EmptyReason::OutsideSyncDepth { months } => {
            page.set_icon_name(Some(WINDOWED_ICON));
            page.set_title(&l10n::mailbox_empty_windowed(i64::from(*months)));
            page.set_description(Some(l10n::mailbox_empty_windowed_body()));
            // A statement the user cannot act on is half the value, so the page carries the way
            // to the depth setting, as the search horizon strip does.
            let change = gtk::Button::with_label(l10n::mailbox_empty_change());
            change.add_css_class("pill");
            change.add_css_class("suggested-action");
            change.set_halign(gtk::Align::Center);
            let input = sender.clone();
            change.connect_clicked(move |_| input.emit(AppInput::OpenSyncDepthSettings));
            page.set_child(Some(&change));
        }
    }
    list.set_placeholder(Some(&page));
}
