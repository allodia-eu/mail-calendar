//! Settings → Accounts: every account, mail or not, from the core's one snapshot, each opening a
//! page of its own (`docs/accounts.md` rule 11).

use adw::prelude::*;
use mailcal_bindings::AccountEntry;

use super::{PageContext, account_page, account_rows, page_box};
use crate::{
    l10n,
    ui::{AppInput, account_settings::AccountsInput},
};

pub(super) fn accounts(ctx: &PageContext) -> gtk::Box {
    let snapshot = ctx.app.accounts_snapshot();
    if let Some(entry) = ctx
        .account
        .as_deref()
        .and_then(|id| snapshot.accounts.iter().find(|entry| entry.id == id))
    {
        return account_page::page(ctx, entry);
    }
    let content = page_box(l10n::settings_category_accounts());
    let add = gtk::Button::with_label(l10n::action_add_account());
    add.add_css_class("suggested-action");
    add.set_halign(gtk::Align::Start);
    let sender = ctx.sender.clone();
    add.connect_clicked(move |_| sender.emit(AppInput::OpenAccountSetup));
    content.append(&add);
    // What the person's other devices have to say, above their own accounts: an offer becomes one
    // of them, and it is drawn even when this device has none; which is exactly when an offer is
    // worth the most.
    if let Some(section) = super::allodia_sync::allodia_sync(ctx) {
        content.append(&section);
    }
    if snapshot.accounts.is_empty() {
        let empty = gtk::Label::new(Some(l10n::settings_accounts_empty()));
        empty.set_xalign(0.0);
        content.append(&empty);
        return content;
    }
    let list = gtk::ListBox::new();
    list.add_css_class("boxed-list");
    list.set_selection_mode(gtk::SelectionMode::None);
    for entry in &snapshot.accounts {
        list.append(&account_row(entry, &ctx.sender));
    }
    content.append(&list);
    content
}

/// One account in the list: its address, then its kind, uses and links, and an arrow to its page.
fn account_row(entry: &AccountEntry, sender: &relm4::Sender<AppInput>) -> adw::ActionRow {
    let row = adw::ActionRow::new();
    // An address and a server's name are not ours to mark up.
    row.set_use_markup(false);
    row.set_title(&entry.address);
    row.set_subtitle(&account_rows::summary(entry));
    row.set_subtitle_lines(0);
    row.add_suffix(&gtk::Image::from_icon_name(crate::ui::icons::NEXT));
    row.set_activatable(true);
    let sender = sender.clone();
    let id = entry.id.clone();
    row.connect_activated(move |_| {
        sender.emit(AppInput::Accounts(AccountsInput::Show(Some(id.clone()))));
    });
    row
}

#[cfg(test)]
pub(crate) mod tests {
    use mailcal_bindings::{AccountEntry, AccountKind};

    use super::account_row;
    use crate::ui::{AppInput, account_settings::AccountsInput};

    pub(crate) fn an_account_row_opens_its_page() {
        let (sender, receiver) = relm4::channel();
        let entry = AccountEntry {
            id: "alice@dav:cloud.example".to_owned(),
            address: "alice@cloud.example".to_owned(),
            kind: AccountKind::Dav,
            uses: Vec::new(),
            links: mailcal_bindings::AccountLinksView::default(),
            linked_from: Vec::new(),
            link_candidates: mailcal_bindings::LinkCandidates::default(),
            endpoints: None,
        };
        let row = account_row(&entry, &sender);
        adw::prelude::ActionRowExt::activate(&row);
        assert!(matches!(
            receiver.recv_sync(),
            Some(AppInput::Accounts(AccountsInput::Show(Some(id)))) if id == entry.id
        ));
    }
}
