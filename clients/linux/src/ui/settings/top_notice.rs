//! The notice at the top of Settings (`docs/notices.md` rule 2): the first one drawn on every
//! screen, through a second banner of the window's own kind. Settings stands over the window's
//! banner, so a setting that could not be saved is said where it was changed.

use adw::prelude::*;
use mailcal_bindings::Notice;

use super::SettingsWindow;
use crate::ui::{AppInput, PrimaryView, notice_bar::NoticeBar, notice_rules};

impl SettingsWindow {
    /// The window's pages under the notice banner, which outlives a redraw of the pages.
    pub(super) fn under_notice(
        &mut self,
        pages: &gtk::Stack,
        sender: &relm4::Sender<AppInput>,
    ) -> gtk::Box {
        let bar = self
            .top_notice
            .get_or_insert_with(|| NoticeBar::quiet(sender));
        if let Some(before) = bar.widget().parent().and_downcast::<gtk::Box>() {
            before.remove(bar.widget());
        }
        let frame = gtk::Box::new(gtk::Orientation::Vertical, 0);
        frame.append(bar.widget());
        frame.append(pages);
        frame
    }

    /// Draws the notice Settings carries, from every notice in force and the account whose page
    /// is open.
    pub(in crate::ui) fn show_notice(&mut self, notices: &[Notice], account: Option<&str>) {
        let Some(bar) = &mut self.top_notice else {
            return;
        };
        let shown: Vec<Notice> = notice_rules::in_settings(notices, account)
            .into_iter()
            .cloned()
            .collect();
        // Every screen's notice belongs to any screen, so which one is named here is moot.
        bar.render(&shown, PrimaryView::Mail);
    }
}
