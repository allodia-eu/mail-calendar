//! Which way a Gmail account's mail syncs in this launch, on its page (`docs/settings.md` row 9):
//! Optimised, Fallback, Can't connect while its mail cannot be reached, or Checking until the
//! first read has settled it. The status never names a
//! protocol; the info button opens a popover with what it means and a link to the help page.

use adw::prelude::*;
use mailcal_bindings::{HelpPage, MailSyncTechnology, help_page_url};

use crate::{l10n, ui::icons};

/// What `technology` is called.
pub(super) fn name(technology: MailSyncTechnology) -> &'static str {
    match technology {
        MailSyncTechnology::Checking => l10n::settings_account_sync_technology_checking(),
        MailSyncTechnology::Optimised => l10n::settings_account_sync_technology_optimised(),
        MailSyncTechnology::Fallback => l10n::settings_account_sync_technology_fallback(),
        MailSyncTechnology::Unreachable => l10n::settings_account_sync_technology_unreachable(),
    }
}

/// What `technology` means, in the popover.
pub(super) fn detail(technology: MailSyncTechnology) -> &'static str {
    match technology {
        MailSyncTechnology::Checking => l10n::settings_account_sync_technology_checking_detail(),
        MailSyncTechnology::Optimised => l10n::settings_account_sync_technology_optimised_detail(),
        MailSyncTechnology::Fallback => l10n::settings_account_sync_technology_fallback_detail(),
        MailSyncTechnology::Unreachable => {
            l10n::settings_account_sync_technology_unreachable_detail()
        }
    }
}

/// The libadwaita style class that colours the status: success green, warning orange, and dimmed
/// while it is still being checked.
fn style_class(technology: MailSyncTechnology) -> &'static str {
    match technology {
        MailSyncTechnology::Checking => "dim-label",
        MailSyncTechnology::Optimised => "success",
        MailSyncTechnology::Fallback => "warning",
        MailSyncTechnology::Unreachable => "error",
    }
}

/// The section, one row: the heading, the status, and the info button.
pub(super) fn group(technology: MailSyncTechnology) -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::new();
    let row = adw::ActionRow::builder()
        .title(
            gtk::glib::markup_escape_text(l10n::settings_account_sync_technology_heading())
                .as_str(),
        )
        .build();

    let status = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    status.set_valign(gtk::Align::Center);
    // The colour repeats the word beside it, so assistive technology reads only the word. The
    // role is construct-only in GTK, which is why the builder sets it.
    let dot = gtk::Label::builder()
        .label("●")
        .accessible_role(gtk::AccessibleRole::Presentation)
        .build();
    dot.add_css_class(style_class(technology));
    status.append(&dot);
    let label = gtk::Label::new(Some(name(technology)));
    label.set_widget_name("account-sync-technology");
    status.append(&label);
    row.add_suffix(&status);

    let explanation = gtk::Box::new(gtk::Orientation::Vertical, 8);
    explanation.set_margin_top(10);
    explanation.set_margin_bottom(10);
    explanation.set_margin_start(10);
    explanation.set_margin_end(10);
    let text = gtk::Label::new(Some(detail(technology)));
    text.set_wrap(true);
    text.set_max_width_chars(36);
    text.set_xalign(0.0);
    explanation.append(&text);
    let url = help_page_url(HelpPage::OptimiseGmailSync);
    let learn_more =
        gtk::LinkButton::with_label(&url, l10n::settings_account_sync_technology_learn_more());
    learn_more.set_halign(gtk::Align::Start);
    explanation.append(&learn_more);
    let popover = gtk::Popover::builder().child(&explanation).build();

    let info = gtk::MenuButton::builder()
        .icon_name(icons::ACCOUNT_SYNC_TECHNOLOGY_ABOUT)
        .popover(&popover)
        .valign(gtk::Align::Center)
        .tooltip_text(l10n::settings_account_sync_technology_about())
        .build();
    info.add_css_class("flat");
    info.update_property(&[gtk::accessible::Property::Label(
        l10n::settings_account_sync_technology_about(),
    )]);
    row.add_suffix(&info);
    group.add(&row);
    group
}

#[cfg(test)]
mod tests {
    use mailcal_bindings::MailSyncTechnology;

    use super::{detail, name};

    #[test]
    fn each_technology_has_a_word_and_an_explanation_of_its_own() {
        let all = [
            MailSyncTechnology::Checking,
            MailSyncTechnology::Optimised,
            MailSyncTechnology::Fallback,
            MailSyncTechnology::Unreachable,
        ];
        let names: std::collections::BTreeSet<_> = all.into_iter().map(name).collect();
        let details: std::collections::BTreeSet<_> = all.into_iter().map(detail).collect();
        assert_eq!(names.len(), all.len());
        assert_eq!(details.len(), all.len());
    }
}
