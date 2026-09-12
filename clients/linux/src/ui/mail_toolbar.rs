//! The mail window's top row.

use adw::prelude::*;
use gtk::accessible::Property as AccessibleProperty;

use super::AppInput;
use crate::l10n;

pub(crate) struct MailToolbar {
    root: adw::HeaderBar,
    #[cfg(test)]
    compose: gtk::Button,
}

impl MailToolbar {
    pub(crate) fn new(sender: &relm4::Sender<AppInput>, search: &gtk::SearchEntry) -> Self {
        let root = adw::HeaderBar::new();
        root.set_show_start_title_buttons(false);
        root.set_show_end_title_buttons(true);
        root.set_title_widget(Some(search));

        let content = adw::ButtonContent::new();
        content.set_icon_name("mail-message-new-symbolic");
        content.set_label(l10n::action_compose());
        let compose = gtk::Button::new();
        compose.set_child(Some(&content));
        compose.set_tooltip_text(Some(l10n::action_compose()));
        compose.update_property(&[AccessibleProperty::Label(l10n::action_compose())]);
        let input = sender.clone();
        compose.connect_clicked(move |_| input.emit(AppInput::BeginNew));
        root.pack_start(&compose);

        Self {
            root,
            #[cfg(test)]
            compose,
        }
    }

    pub(crate) fn widget(&self) -> &adw::HeaderBar {
        &self.root
    }
}

#[cfg(test)]
#[path = "mail_toolbar_tests.rs"]
pub(super) mod tests;
