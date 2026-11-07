//! The notices about an account, at the top of its page (`docs/notices.md` rule 6). Said here, not
//! acted on: the page's own sections below are where each remedy is. One about something that
//! already happened can be closed here as it can on the banner.

use adw::prelude::*;
use mailcal_bindings::{Notice, NoticeSeverity};

use super::PageContext;
use crate::{
    l10n,
    ui::{AppInput, notice_rules, notices::NoticeInput},
};

/// The notices about `account`, in the core's order, from the snapshot.
pub(super) fn of(app: &mailcal_bindings::MailcalApp, account: &str) -> Vec<Notice> {
    notice_rules::about(&app.notices().notices, account)
        .into_iter()
        .cloned()
        .collect()
}

/// One row per notice about `account`, or `None` when there is none.
pub(super) fn group(ctx: &PageContext, account: &str) -> Option<adw::PreferencesGroup> {
    let notices = of(&ctx.app, account);
    if notices.is_empty() {
        return None;
    }
    let section = adw::PreferencesGroup::new();
    for notice in &notices {
        section.add(&row(notice, &ctx.sender));
    }
    Some(section)
}

fn row(notice: &Notice, sender: &relm4::Sender<AppInput>) -> adw::ActionRow {
    let row = adw::ActionRow::new();
    // An address is the person's own text, not markup.
    row.set_use_markup(false);
    row.set_title(&notice_rules::words(&notice.title));
    row.set_title_lines(0);
    if let Some(beneath) = notice_rules::beneath(notice) {
        row.set_subtitle(&beneath);
    }
    let glyph = gtk::Image::from_icon_name(notice_rules::glyph(notice.severity));
    if notice.severity == NoticeSeverity::Blocked {
        glyph.add_css_class("error");
    }
    glyph.set_accessible_role(gtk::AccessibleRole::Presentation);
    row.add_prefix(&glyph);
    if notice.dismissible {
        let close = gtk::Button::with_label(l10n::notices_dismiss());
        close.set_valign(gtk::Align::Center);
        let (sender, id) = (sender.clone(), notice.id.clone());
        close.connect_clicked(move |_| {
            sender.emit(AppInput::Notice(NoticeInput::Dismiss(id.clone())));
        });
        row.add_suffix(&close);
    }
    row
}
