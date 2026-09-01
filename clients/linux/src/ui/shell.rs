//! Three-pane GTK/libadwaita shell and view rendering.

use std::{cell::Cell, rc::Rc, time::Duration};

use adw::prelude::*;
use gtk::accessible::Property as AccessibleProperty;

use super::{
    AppInput, AppModel, PrimaryView,
    calendar::CalendarPane,
    composer::ComposerPane,
    composer_draft::DiscardDraftDialog,
    connectivity::ConnectivityBanners,
    contacts::ContactsPane,
    destinations::DestinationBar,
    folder_pane::{self, FolderPaneRendering, FolderPaneSelection},
    invitation::ReplyPromptDialog,
    mail_actions::{self, PermanentDeleteDialog},
    mailbox::MailboxRendering,
    mailbox_progressive::ProgressiveRenderer,
    reading::{InvitationClock, ReadingPane},
    search::SearchBar,
    selection_bar::SelectionBar,
    selection_input::{selection_gesture, selection_keys, sync_selection},
    settings::SettingsWindow,
    setup::SetupWindow,
    time_zone::TimeZonePrompt,
    unfiled_copy::UnfiledCopyPrompt,
    welcome::WelcomeWindow,
};
use crate::{l10n, preferences};

pub(crate) struct AppWidgets {
    root: adw::ApplicationWindow,
    subtitle: adw::WindowTitle,
    sidebar: gtk::ListBox,
    messages: gtk::ListBox,
    search: SearchBar,
    selection_bar: SelectionBar,
    primary: gtk::Stack,
    detail: gtk::Stack,
    destinations: DestinationBar,
    calendar: CalendarPane,
    contacts: ContactsPane,
    reading: ReadingPane,
    composer: ComposerPane,
    connectivity: ConnectivityBanners,
    notice: adw::Banner,
    sync_strip: gtk::Box,
    sync_bar_row: gtk::Box,
    sync_progress: gtk::ProgressBar,
    sync_caption: gtk::Label,
    sync_indeterminate: Rc<Cell<bool>>,
    sync_hint: gtk::Label,
    settings: SettingsWindow,
    setup: SetupWindow,
    welcome: WelcomeWindow,
    time_zone: TimeZonePrompt,
    unfiled_copy: UnfiledCopyPrompt,
    /// The standing "the organiser wasn't told" question. A dialog rather than a banner: it
    /// carries two answers and a tick, and it may not be dismissed without one of them.
    reply_prompt: ReplyPromptDialog,
    mail_delete: PermanentDeleteDialog,
    discard_draft: DiscardDraftDialog,
    sender: relm4::Sender<AppInput>,
    rendered_snapshot: Option<MailboxRendering>,
    mailbox_renderer: ProgressiveRenderer,
    rendered_pane: Option<FolderPaneRendering>,
    rendered_sidebar_selection: FolderPaneSelection,
    rendered_primary: Option<PrimaryView>,
}

impl AppWidgets {
    pub(super) fn new(root: adw::ApplicationWindow, sender: relm4::Sender<AppInput>) -> Self {
        root.set_title(Some(l10n::app_title()));
        root.set_default_width(1280);
        root.set_default_height(800);
        let notice = adw::Banner::new("");
        // The banner now also carries host error strings (a failed account removal), and
        // `AdwBanner:title` is Pango markup by default: an ampersand would render it blank.
        notice.set_use_markup(false);
        let unfiled_copy = UnfiledCopyPrompt::new(&sender);
        let shell = gtk::Box::new(gtk::Orientation::Vertical, 0);
        shell.append(&notice);
        let connectivity = ConnectivityBanners::new(&sender);
        shell.append(connectivity.widget());

        // Every row carries its own handler (folder_pane), so the list needs none: a folder key
        // is unique only within its account, and this pane holds every account's tree.
        let sidebar = gtk::ListBox::new();
        sidebar.set_selection_mode(gtk::SelectionMode::Single);
        sidebar.add_css_class("navigation-sidebar");
        sidebar.update_property(&[AccessibleProperty::Label(l10n::sidebar_accounts())]);
        let sidebar_scroll = gtk::ScrolledWindow::new();
        sidebar_scroll.set_min_content_width(folder_pane::width::MIN);
        sidebar_scroll.set_child(Some(&sidebar));
        let destinations = DestinationBar::new(&sender);
        let sidebar_toolbar = sidebar_pane(&sender, &sidebar_scroll, &destinations);

        let messages = gtk::ListBox::new();
        // Multiple, so a selection is the platform's own selected state rather than a colour we
        // paint: a screen reader reads it, and Shift+arrow extends it (`docs/list-selection.md`,
        // rule 11). The widget still holds no selection of its own; `sync_selection` draws the
        // model's onto it after every render.
        messages.set_selection_mode(gtk::SelectionMode::Multiple);
        messages.add_css_class("boxed-list");
        messages.update_property(&[AccessibleProperty::Label(l10n::a11y_message_list())]);
        selection_gesture(&messages, &sender);
        selection_keys(&messages, &sender);
        let selection_bar = SelectionBar::new(&sender);
        let message_scroll = gtk::ScrolledWindow::new();
        message_scroll.set_min_content_width(340);
        message_scroll.set_child(Some(&messages));
        let list_toolbar = adw::ToolbarView::new();
        let list_header = adw::HeaderBar::new();
        list_header.set_show_start_title_buttons(false);
        list_header.set_show_end_title_buttons(false);
        let subtitle = adw::WindowTitle::new(l10n::sidebar_all_inboxes(), "");
        list_header.set_title_widget(Some(&subtitle));
        let compose = gtk::Button::from_icon_name("mail-message-new-symbolic");
        compose.set_tooltip_text(Some(l10n::action_compose()));
        compose.update_property(&[AccessibleProperty::Label(l10n::action_compose())]);
        let input = sender.clone();
        compose.connect_clicked(move |_| input.emit(AppInput::BeginNew));
        list_header.pack_start(&compose);
        let refresh = gtk::Button::from_icon_name("view-refresh-symbolic");
        refresh.set_tooltip_text(Some(l10n::action_refresh()));
        refresh.update_property(&[AccessibleProperty::Label(l10n::action_refresh())]);
        let input = sender.clone();
        refresh.connect_clicked(move |_| input.emit(AppInput::RefreshRequested));
        list_header.pack_end(&refresh);
        list_toolbar.add_top_bar(&list_header);
        // Under the header rather than in it: the field is one of three things the header would
        // then hold in a pane the user can drag to 260 px, and the scope filter and horizon line
        // it reveals need the full width anyway.
        let search = SearchBar::new(&sender);
        list_toolbar.add_top_bar(search.widget());
        // Under the search chrome and over the rows, which is where the count belongs: it
        // describes the list beneath it, and the revealer keeps the list's top edge from jumping
        // as the selection empties and fills.
        list_toolbar.add_top_bar(selection_bar.widget());
        list_toolbar.set_content(Some(&message_scroll));
        // One strip under the list: the foreground bar may take a row because the user awaits it;
        // the background hint borrows that same location and never moves the list's top edge.
        let sync_strip = gtk::Box::new(gtk::Orientation::Vertical, 0);
        let sync_bar_row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        sync_bar_row.set_margin_top(6);
        sync_bar_row.set_margin_bottom(6);
        sync_bar_row.set_margin_start(12);
        sync_bar_row.set_margin_end(12);
        let sync_progress = gtk::ProgressBar::new();
        sync_progress.set_hexpand(true);
        sync_progress.set_valign(gtk::Align::Center);
        sync_progress.set_pulse_step(0.08);
        let sync_caption = gtk::Label::new(None);
        sync_caption.add_css_class("dim-label");
        sync_caption.add_css_class("caption");
        sync_bar_row.append(&sync_progress);
        sync_bar_row.append(&sync_caption);
        sync_bar_row.set_visible(false);
        sync_strip.append(&sync_bar_row);

        // Plain text because the caption carries an account address; an ampersand in one must
        // render, not fail a markup parse.
        let sync_hint = gtk::Label::new(None);
        sync_hint.set_xalign(0.0);
        sync_hint.set_ellipsize(gtk::pango::EllipsizeMode::End);
        sync_hint.set_margin_top(6);
        sync_hint.set_margin_bottom(6);
        sync_hint.set_margin_start(12);
        sync_hint.set_margin_end(12);
        sync_hint.add_css_class("dim-label");
        sync_hint.add_css_class("caption");
        sync_hint.set_visible(false);
        sync_strip.append(&sync_hint);
        sync_strip.set_visible(false);
        list_toolbar.add_bottom_bar(&sync_strip);
        let sync_indeterminate = Rc::new(Cell::new(false));
        let pulsing = Rc::clone(&sync_indeterminate);
        let progress = sync_progress.clone();
        gtk::glib::timeout_add_local(Duration::from_millis(120), move || {
            if pulsing.get() && progress.is_visible() {
                progress.pulse();
            }
            gtk::glib::ControlFlow::Continue
        });

        let reading = ReadingPane::new(&root, sender.clone());
        let composer = ComposerPane::new();
        let detail = gtk::Stack::new();
        detail.set_hexpand(true);
        detail.set_vexpand(true);
        detail.add_named(reading.widget(), Some("reading"));
        detail.add_named(composer.widget(), Some("composer"));
        detail.set_visible_child_name("reading");

        let inner = gtk::Paned::new(gtk::Orientation::Horizontal);
        inner.set_start_child(Some(&list_toolbar));
        inner.set_end_child(Some(&detail));
        inner.set_position(390);
        inner.set_resize_start_child(false);
        inner.set_shrink_start_child(false);
        let calendar = CalendarPane::new(&root, sender.clone());
        let contacts = ContactsPane::new(&root, sender.clone());
        let primary = gtk::Stack::new();
        primary.set_transition_type(gtk::StackTransitionType::Crossfade);
        primary.add_named(&inner, Some("mail"));
        primary.add_named(calendar.widget(), Some("calendar"));
        primary.add_named(contacts.widget(), Some("contacts"));
        let outer = gtk::Paned::new(gtk::Orientation::Horizontal);
        outer.set_start_child(Some(&sidebar_toolbar));
        outer.set_end_child(Some(&primary));
        outer.set_resize_start_child(false);
        outer.set_shrink_start_child(false);
        outer.set_vexpand(true);
        outer.update_property(&[AccessibleProperty::Label(l10n::a11y_resize_folder_pane())]);
        restore_pane_width(&outer, &root);
        shell.append(&outer);
        root.set_content(Some(&shell));

        Self {
            root,
            subtitle,
            sidebar,
            messages,
            search,
            selection_bar,
            primary,
            detail,
            destinations,
            calendar,
            contacts,
            reading,
            composer,
            connectivity,
            notice,
            sync_strip,
            sync_bar_row,
            sync_progress,
            sync_caption,
            sync_indeterminate,
            sync_hint,
            settings: SettingsWindow::default(),
            setup: SetupWindow::default(),
            welcome: WelcomeWindow::default(),
            time_zone: TimeZonePrompt::default(),
            unfiled_copy,
            reply_prompt: ReplyPromptDialog::new(),
            mail_delete: PermanentDeleteDialog::default(),
            discard_draft: DiscardDraftDialog::default(),
            sender,
            rendered_snapshot: None,
            mailbox_renderer: ProgressiveRenderer::default(),
            rendered_pane: None,
            rendered_sidebar_selection: FolderPaneSelection::default(),
            rendered_primary: None,
        }
    }

    pub(super) fn render(&mut self, model: &AppModel) {
        let calendar_opened = entered_calendar(self.rendered_primary, model.primary);
        self.destinations.sync(model.primary);
        match model.primary {
            PrimaryView::Mail => self.primary.set_visible_child_name("mail"),
            PrimaryView::Calendar => {
                self.calendar.render(&model.calendar, model.app.as_ref());
                self.calendar.render_manager(
                    model.calendar_manager_generation,
                    model.app.as_ref(),
                    &model.snapshot.accounts,
                );
                self.primary.set_visible_child_name("calendar");
                if calendar_opened {
                    self.calendar.opened();
                }
            }
            PrimaryView::Contacts => {
                self.contacts.render(&model.contacts);
                self.primary.set_visible_child_name("contacts");
            }
        }
        self.rendered_primary = Some(model.primary);
        self.subtitle.set_title(&model.list_title());
        self.subtitle.set_subtitle(&model.subtitle());
        self.search.render(&model.search, &model.snapshot);
        // Two keys, not one: opening an account's tree must not rebuild the message list, which
        // would replace an open conversation's row mid-disclosure.
        let pane =
            FolderPaneRendering::new(&model.snapshot, &model.connectivity.unreachable_accounts);
        if self.rendered_pane.as_ref() != Some(&pane) {
            folder_pane::render(
                &self.sidebar,
                &model.snapshot,
                &model.connectivity.unreachable_accounts,
                &self.sender,
            );
            self.rendered_pane = Some(pane);
        }
        self.rendered_sidebar_selection
            .sync(&self.sidebar, &model.snapshot);
        let display_zone = model.calendar.display_zone();
        let rendering = MailboxRendering::new(&model.snapshot, display_zone);
        if self.rendered_snapshot.as_ref() != Some(&rendering) {
            self.mailbox_renderer.render(
                &self.messages,
                &model.snapshot,
                &model.expanded_threads,
                mail_actions::in_junk_folder(&model.snapshot),
                display_zone,
                &self.sender,
            );
            self.rendered_snapshot = Some(rendering);
        }
        // After the rows, always: a plain click has already moved the widget's own selection, and
        // this is what brings it back to what the model says (`selection_gesture`).
        sync_selection(&self.messages, model);
        self.selection_bar
            .render(model.selection.summary(&model.snapshot.rows));
        if let Some(request) = &model.composer {
            self.settings.close();
            if !self.composer.is_active(model.composer_generation) {
                self.reading.suspend();
                let accounts = model
                    .snapshot
                    .accounts
                    .iter()
                    .map(|account| (account.id.clone(), account.email.clone()))
                    .collect::<Vec<_>>();
                self.composer.show(
                    model.composer_generation,
                    request,
                    &accounts,
                    model.app.as_ref(),
                    &self.root,
                    self.sender.clone(),
                );
            }
            if model.composer_error {
                self.composer.show_error();
            }
            // A navigation is waiting on this draft's answer. Issued from here because the model
            // renders behind a shared reference and cannot run the editor round trip itself.
            if let Some(generation) = model.draft_check {
                self.composer.check_draft(generation, &self.sender);
            }
            self.detail.set_visible_child_name("composer");
        } else {
            self.composer.teardown();
            self.reading.render(
                &model.reading,
                model.app.as_ref(),
                model.webview_available,
                InvitationClock {
                    zone: model.calendar.display_zone(),
                    use_24_hour: model.calendar.uses_24_hour(),
                    write_status: model.calendar.write_status(),
                    generation: model.reading_generation,
                },
                &self.sender,
            );
            self.detail.set_visible_child_name("reading");
        }
        self.notice
            .set_title(model.notice.as_deref().unwrap_or_default());
        self.notice.set_revealed(model.notice.is_some());
        self.connectivity.render(&model.connectivity, model.primary);
        if let Some(bar) = &model.sync_bar {
            self.sync_caption.set_text(&bar.caption);
            self.sync_indeterminate.set(bar.fraction.is_none());
            self.sync_progress.set_fraction(bar.fraction.unwrap_or(0.0));
            self.sync_bar_row.set_visible(true);
            self.sync_hint.set_visible(false);
        } else {
            self.sync_indeterminate.set(false);
            self.sync_bar_row.set_visible(false);
            self.sync_hint
                .set_text(model.sync_hint.as_deref().unwrap_or_default());
            self.sync_hint.set_visible(model.sync_hint.is_some());
        }
        self.sync_strip
            .set_visible(model.sync_bar.is_some() || model.sync_hint.is_some());
        if model.draft_check.is_some() {
            self.settings.close();
        }
        self.settings.render(
            model.settings.render_state(
                model.credential_repair_failed.as_deref(),
                &model.allodia.accounts_synced,
            ),
            &self.root,
            model.app.as_ref(),
            model.preferences.clone(),
            self.sender.clone(),
        );
        // Dismiss the required welcome window before presenting required account setup. Both
        // reject user close requests, so this ordering is part of the modal-lifecycle contract.
        self.welcome.render(
            model.host_tasks.welcome_pending,
            &self.root,
            model.app.as_deref(),
            self.sender.clone(),
        );
        self.time_zone.render(
            (!model.host_tasks.welcome_pending)
                .then_some(model.app.as_deref())
                .flatten(),
            &self.root,
            &self.sender,
        );
        self.setup.render(&model.setup, &self.root, &self.sender);
        self.unfiled_copy
            .render(model.unfiled_copy.as_ref(), &self.root);
        self.reply_prompt.render(
            model.reply_prompt.as_ref(),
            model.reply_prompt_generation,
            &self.root,
            &self.sender,
        );
        self.mail_delete
            .render(model.pending_mail_delete.as_ref(), &self.root, &self.sender);
        self.discard_draft
            .render(model.discard_prompt, &self.root, &self.sender);
    }
}

fn entered_calendar(previous: Option<PrimaryView>, current: PrimaryView) -> bool {
    current == PrimaryView::Calendar && previous != Some(PrimaryView::Calendar)
}

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
fn restore_pane_width(pane: &gtk::Paned, root: &adw::ApplicationWindow) {
    let stored = preferences::global()
        .folder_pane_width()
        .unwrap_or(folder_pane::width::DEFAULT);
    pane.set_position(folder_pane::width::clamp(stored, root.default_width()));
    pane.connect_position_notify(|pane| {
        preferences::global().set_folder_pane_width(pane.position());
    });
}

#[cfg(test)]
mod tests {
    use super::{PrimaryView, entered_calendar};

    #[test]
    fn only_an_entry_to_the_calendar_requests_recentering() {
        assert!(entered_calendar(None, PrimaryView::Calendar));
        assert!(entered_calendar(
            Some(PrimaryView::Mail),
            PrimaryView::Calendar
        ));
        assert!(!entered_calendar(
            Some(PrimaryView::Calendar),
            PrimaryView::Calendar
        ));
        assert!(!entered_calendar(
            Some(PrimaryView::Calendar),
            PrimaryView::Mail
        ));
    }
}
