//! The contacts surface: an A–Z list of unified people beside one person's detail.
//!
//! Every row here is one **person**, not one provider card; the core has already merged the cards
//! that share an address, across accounts. A merged row says so, which is a product rule rather
//! than a decoration: a user who filed a contact twice and now sees it once must be able to find
//! out why (`docs/contacts.md` §1). The detail's "Also in" is that explanation.
//!
//! Read-only in this release, and it says so in as many words rather than offering an edit
//! affordance that does nothing (§3).

use adw::prelude::*;
use gtk::accessible::Property as AccessibleProperty;

use super::{
    super::{AppInput, avatar, destinations::CONTACTS_ICON, mailbox},
    model::{ContactsModel, ListState, PersonDetail, PersonRow, ValueGroup},
};
use crate::l10n;

pub(crate) struct ContactsPane {
    root: gtk::Paned,
    search: gtk::SearchEntry,
    /// Kept so [`ContactsPane::render`] can put the model's query in the box **without** it: the
    /// query is the core's, and echoing it back as a fresh search would dispatch a narrowing
    /// nobody typed.
    search_handler: gtk::glib::SignalHandlerId,
    list: gtk::ListBox,
    list_stack: gtk::Stack,
    empty: adw::StatusPage,
    detail: gtk::Box,
    detail_stack: gtk::Stack,
    sender: relm4::Sender<AppInput>,
    rendered_rows: Option<Vec<PersonRow>>,
    rendered_detail: Option<PersonDetail>,
}

impl ContactsPane {
    pub(crate) fn new(sender: relm4::Sender<AppInput>) -> Self {
        mailbox::install_styles();

        let header = adw::HeaderBar::new();
        header.set_show_start_title_buttons(false);
        header.set_show_end_title_buttons(false);
        header.set_title_widget(Some(&adw::WindowTitle::new(l10n::contacts_title(), "")));
        let refresh = gtk::Button::from_icon_name("view-refresh-symbolic");
        refresh.set_tooltip_text(Some(l10n::action_refresh()));
        refresh.update_property(&[AccessibleProperty::Label(l10n::action_refresh())]);
        let input = sender.clone();
        refresh.connect_clicked(move |_| input.emit(AppInput::RefreshContacts));
        header.pack_end(&refresh);

        // The narrowing runs in the **core**, over name, email, phone, organisation and title, so
        // every client narrows identically; and a person beyond the loaded page is still
        // findable, which filtering the rows already on screen could never manage.
        let search = gtk::SearchEntry::new();
        search.set_placeholder_text(Some(l10n::contacts_search_placeholder()));
        search.update_property(&[AccessibleProperty::Label(
            l10n::contacts_search_placeholder(),
        )]);
        search.set_margin_top(6);
        search.set_margin_bottom(6);
        search.set_margin_start(12);
        search.set_margin_end(12);
        let input = sender.clone();
        let search_handler = search.connect_search_changed(move |entry| {
            input.emit(AppInput::SearchContacts(entry.text().to_string()));
        });

        let list = gtk::ListBox::new();
        list.set_selection_mode(gtk::SelectionMode::Single);
        list.add_css_class("navigation-sidebar");
        list.update_property(&[AccessibleProperty::Label(l10n::contacts_title())]);
        let list_scroll = gtk::ScrolledWindow::new();
        list_scroll.set_child(Some(&list));
        let empty = adw::StatusPage::new();
        empty.set_icon_name(Some(CONTACTS_ICON));
        let list_stack = gtk::Stack::new();
        list_stack.add_named(&list_scroll, Some("rows"));
        list_stack.add_named(&empty, Some("empty"));
        let list_toolbar = adw::ToolbarView::new();
        list_toolbar.add_top_bar(&header);
        list_toolbar.add_top_bar(&search);
        list_toolbar.set_content(Some(&list_stack));

        let detail = gtk::Box::new(gtk::Orientation::Vertical, 18);
        // A wrapping label asks for a narrow natural width, and a scrolled window grants exactly
        // what its child asks for; so without this the name breaks over two lines beside acres of
        // empty pane.
        detail.set_hexpand(true);
        detail.set_margin_top(24);
        detail.set_margin_bottom(24);
        detail.set_margin_start(24);
        detail.set_margin_end(24);
        let detail_scroll = gtk::ScrolledWindow::new();
        detail_scroll.set_child(Some(&detail));
        // The counterpart of the reading pane's placeholder, so the second column is never a
        // blank half-window before a person is picked.
        let placeholder = adw::StatusPage::new();
        placeholder.set_icon_name(Some(CONTACTS_ICON));
        placeholder.set_title(l10n::contacts_title());
        let detail_stack = gtk::Stack::new();
        detail_stack.add_named(&detail_scroll, Some("person"));
        detail_stack.add_named(&placeholder, Some("placeholder"));
        detail_stack.set_visible_child_name("placeholder");
        let detail_toolbar = adw::ToolbarView::new();
        // The rightmost pane carries the window controls, as the reading pane does beside the
        // mail list.
        detail_toolbar.add_top_bar(&adw::HeaderBar::new());
        detail_toolbar.set_content(Some(&detail_stack));

        let root = gtk::Paned::new(gtk::Orientation::Horizontal);
        root.set_start_child(Some(&list_toolbar));
        root.set_end_child(Some(&detail_toolbar));
        root.set_position(390);
        root.set_resize_start_child(false);
        root.set_shrink_start_child(false);

        Self {
            root,
            search,
            search_handler,
            list,
            list_stack,
            empty,
            detail,
            detail_stack,
            sender,
            rendered_rows: None,
            rendered_detail: None,
        }
    }

    pub(crate) fn widget(&self) -> &gtk::Paned {
        &self.root
    }

    pub(crate) fn render(&mut self, model: &ContactsModel) {
        if self.search.text() != model.query() {
            self.search.block_signal(&self.search_handler);
            self.search.set_text(model.query());
            self.search.unblock_signal(&self.search_handler);
        }
        if self.rendered_rows.as_deref() != Some(model.rows()) {
            render_rows(&self.list, model.rows(), &self.sender);
            self.rendered_rows = Some(model.rows().to_vec());
        }
        self.render_state(model.state());
        if self.rendered_detail.as_ref() != model.opened() {
            match model.opened() {
                Some(person) => {
                    render_detail(&self.detail, person);
                    self.detail_stack.set_visible_child_name("person");
                }
                None => self.detail_stack.set_visible_child_name("placeholder"),
            }
            self.rendered_detail = model.opened().cloned();
        }
    }

    fn render_state(&self, state: ListState) {
        match state {
            ListState::Rows => self.list_stack.set_visible_child_name("rows"),
            ListState::NoContacts => {
                self.empty.set_title(l10n::contacts_empty());
                self.empty
                    .set_description(Some(l10n::contacts_empty_body()));
                self.list_stack.set_visible_child_name("empty");
            }
            ListState::NoResults => {
                self.empty.set_title(l10n::contacts_no_results());
                // Nothing under a no-results headline: "they appear here once they have synced"
                // answers a question this user did not ask.
                self.empty.set_description(None);
                self.list_stack.set_visible_child_name("empty");
            }
        }
    }
}

fn render_rows(list: &gtk::ListBox, rows: &[PersonRow], sender: &relm4::Sender<AppInput>) {
    mailbox::clear(list);
    for row in rows {
        if let Some(section) = &row.section {
            list.append(&section_row(section));
        }
        list.append(&person_row(row, sender));
    }
}

/// The A–Z header, as a row of its own that can be neither selected nor activated: it is a label
/// over the people beneath it, not one of them.
fn section_row(section: &str) -> gtk::ListBoxRow {
    let label = gtk::Label::new(Some(section));
    label.set_xalign(0.0);
    label.set_margin_top(12);
    label.set_margin_bottom(2);
    label.set_margin_start(6);
    label.add_css_class("caption");
    label.add_css_class("dim-label");
    let row = gtk::ListBoxRow::new();
    row.set_child(Some(&label));
    row.set_selectable(false);
    row.set_activatable(false);
    row
}

/// One person: the avatar, the name over the primary address, and; only for a real merge; the
/// disclosure that this row is several accounts' cards shown as one.
///
/// The name and the address are the **server's** text, so the row is built plain: a bare ampersand
/// must not be read as an entity, and a markup-shaped name must be shown, never applied.
fn person_row(row: &PersonRow, sender: &relm4::Sender<AppInput>) -> adw::ActionRow {
    let widget = mailbox::plain_text_row();
    widget.set_title(&row.name);
    widget.set_subtitle(&row.email);
    widget.set_title_lines(1);
    widget.set_subtitle_lines(1);
    widget.set_activatable(true);
    widget.add_prefix(&avatar::view(&row.avatar, 34));
    if let Some(accounts) = &row.accounts {
        widget.add_suffix(&mailbox::badge(accounts));
    }
    let input = sender.clone();
    let id = row.id.clone();
    widget.connect_activated(move |_| input.emit(AppInput::OpenContact(id.clone())));
    widget
}

fn render_detail(container: &gtk::Box, person: &PersonDetail) {
    while let Some(child) = container.first_child() {
        container.remove(&child);
    }

    let heading = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    heading.append(&avatar::view(&person.avatar, 56));
    let name = gtk::Label::new(Some(&person.name));
    name.set_xalign(0.0);
    name.set_hexpand(true);
    name.set_wrap(true);
    name.set_selectable(true);
    name.add_css_class("title-2");
    heading.append(&name);
    container.append(&heading);

    for group in &person.groups {
        container.append(&value_group(group));
    }
    if !person.accounts.is_empty() {
        let group = adw::PreferencesGroup::new();
        group.set_title(l10n::contacts_section_accounts());
        for account in &person.accounts {
            let row = mailbox::plain_text_row();
            row.set_title(account);
            group.add(&row);
        }
        container.append(&group);
    }

    // Said in as many words, rather than left for the user to infer from the absence of an edit
    // button; or, worse, from a disabled one they press twice.
    let read_only = gtk::Label::new(Some(l10n::contacts_read_only()));
    read_only.set_xalign(0.0);
    read_only.set_wrap(true);
    read_only.add_css_class("caption");
    read_only.add_css_class("dim-label");
    container.append(&read_only);
}

/// One headed group of values. A value's provenance is its **subtitle**, never a second column:
/// several full addresses joined by commas take whatever width they want and would squeeze the
/// value itself to nothing.
fn value_group(group: &ValueGroup) -> adw::PreferencesGroup {
    let widget = adw::PreferencesGroup::new();
    widget.set_title(group.heading);
    for value in &group.values {
        let row = mailbox::plain_text_row();
        row.set_title(&value.value);
        if !value.accounts.is_empty() {
            row.set_subtitle(&value.accounts);
        }
        widget.add(&row);
    }
    widget
}

#[cfg(test)]
#[path = "pane_tests.rs"]
pub(crate) mod tests;
