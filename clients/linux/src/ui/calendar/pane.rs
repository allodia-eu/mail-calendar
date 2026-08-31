//! libadwaita calendar shell: mode menu, navigation cluster, and surface stack.

use std::sync::Arc;

use adw::prelude::*;
use gtk::accessible::Property as AccessibleProperty;
use mailcal_bindings::{AccountRow, CalendarWriteStatus, MailcalApp};

use super::{
    super::AppInput,
    dialogs,
    grid::GridSurface,
    manager::CalendarManager,
    model::{CalendarMode, CalendarModel},
    views,
};
use crate::l10n;

pub(crate) struct CalendarPane {
    root: adw::ToolbarView,
    title: adw::WindowTitle,
    mode: gtk::MenuButton,
    previous: gtk::Button,
    today: gtk::Button,
    next: gtk::Button,
    new_event: gtk::Button,
    manager: CalendarManager,
    status: gtk::Box,
    status_spinner: gtk::Spinner,
    status_label: gtk::Label,
    status_retry: gtk::Button,
    surfaces: gtk::Stack,
    grid: GridSurface,
    month: gtk::Grid,
    agenda: gtk::ListBox,
    parent: adw::ApplicationWindow,
    sender: relm4::Sender<AppInput>,
    shown_dialog_generation: u64,
}

impl CalendarPane {
    pub(crate) fn new(parent: &adw::ApplicationWindow, sender: relm4::Sender<AppInput>) -> Self {
        let root = adw::ToolbarView::new();
        let header = adw::HeaderBar::new();
        header.set_show_start_title_buttons(false);
        header.set_show_end_title_buttons(true);
        let title = adw::WindowTitle::new(l10n::nav_calendar(), "");
        header.set_title_widget(Some(&title));

        let mode = mode_menu(&sender);
        header.pack_start(&mode);
        let navigation = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        navigation.add_css_class("linked");
        let previous = icon_button("go-previous-symbolic", l10n::calendar_prev_week());
        let today = gtk::Button::with_label(l10n::calendar_today());
        today.update_property(&[AccessibleProperty::Label(l10n::calendar_back_to_today())]);
        let next = icon_button("go-next-symbolic", l10n::calendar_next_week());
        let input = sender.clone();
        previous.connect_clicked(move |_| input.emit(AppInput::StepCalendar(-1)));
        let input = sender.clone();
        today.connect_clicked(move |_| input.emit(AppInput::CalendarToday));
        let input = sender.clone();
        next.connect_clicked(move |_| input.emit(AppInput::StepCalendar(1)));
        navigation.append(&previous);
        navigation.append(&today);
        navigation.append(&next);
        header.pack_start(&navigation);

        let status = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        status.set_valign(gtk::Align::Center);
        let status_spinner = gtk::Spinner::new();
        let status_label = gtk::Label::new(None);
        let status_retry = gtk::Button::with_label(l10n::action_refresh());
        status_retry.add_css_class("flat");
        let input = sender.clone();
        status_retry.connect_clicked(move |_| input.emit(AppInput::RefreshCalendar));
        status.append(&status_spinner);
        status.append(&status_label);
        status.append(&status_retry);
        header.pack_end(&status);
        let new_event = gtk::Button::with_label(l10n::action_new_event());
        new_event.add_css_class("suggested-action");
        new_event.update_property(&[AccessibleProperty::Label(l10n::action_new_event())]);
        let input = sender.clone();
        new_event.connect_clicked(move |_| input.emit(AppInput::BeginNewEvent));
        header.pack_end(&new_event);
        let manage = gtk::Button::with_label(l10n::calendar_manage());
        manage.update_property(&[AccessibleProperty::Label(l10n::calendar_manage())]);
        let input = sender.clone();
        manage.connect_clicked(move |_| input.emit(AppInput::ManageCalendars));
        header.pack_end(&manage);
        root.add_top_bar(&header);

        let grid = GridSurface::new(sender.clone());
        let month = gtk::Grid::new();
        month.set_column_homogeneous(true);
        month.set_row_homogeneous(true);
        month.update_property(&[AccessibleProperty::Label(l10n::calendar_view_month())]);
        let month_scroll = gtk::ScrolledWindow::new();
        month_scroll.set_child(Some(&month));
        let agenda = gtk::ListBox::new();
        agenda.add_css_class("boxed-list");
        agenda.set_selection_mode(gtk::SelectionMode::None);
        agenda.update_property(&[AccessibleProperty::Label(l10n::calendar_view_agenda())]);
        let agenda_scroll = gtk::ScrolledWindow::new();
        agenda_scroll.set_child(Some(&agenda));
        let surfaces = gtk::Stack::new();
        surfaces.set_transition_type(gtk::StackTransitionType::Crossfade);
        surfaces.add_named(&grid.root, Some("grid"));
        surfaces.add_named(&month_scroll, Some("month"));
        surfaces.add_named(&agenda_scroll, Some("agenda"));
        root.set_content(Some(&surfaces));

        Self {
            root,
            title,
            mode,
            previous,
            today,
            next,
            new_event,
            manager: CalendarManager::default(),
            status,
            status_spinner,
            status_label,
            status_retry,
            surfaces,
            grid,
            month,
            agenda,
            parent: parent.clone(),
            sender,
            shown_dialog_generation: 0,
        }
    }

    pub(crate) fn widget(&self) -> &adw::ToolbarView {
        &self.root
    }

    pub(crate) fn opened(&self) {
        self.grid.opened();
    }

    pub(crate) fn render_manager(
        &mut self,
        generation: u64,
        app: Option<&Arc<MailcalApp>>,
        accounts: &[AccountRow],
    ) {
        self.manager.render(generation, &self.parent, app, accounts);
    }

    pub(crate) fn render(&mut self, model: &CalendarModel, app: Option<&Arc<MailcalApp>>) {
        self.title.set_title(&model.period_title());
        self.title.set_subtitle(&model.agenda.timezone);
        self.mode.set_label(mode_label(model.mode));
        self.new_event.set_sensitive(model.can_create());
        let navigation_enabled = model.mode != CalendarMode::Agenda;
        self.previous.set_sensitive(navigation_enabled);
        self.today.set_sensitive(navigation_enabled);
        self.next.set_sensitive(navigation_enabled);
        self.previous
            .set_tooltip_text(Some(&previous_label(model.mode)));
        self.next.set_tooltip_text(Some(&next_label(model.mode)));
        render_status(self, model.write_status);
        match model.mode {
            CalendarMode::Month => {
                views::render_month(&self.month, model, &self.sender);
                self.surfaces.set_visible_child_name("month");
            }
            CalendarMode::Agenda => {
                views::render_agenda(&self.agenda, model, &self.sender);
                self.surfaces.set_visible_child_name("agenda");
            }
            _ => {
                self.grid
                    .render(model, adw::StyleManager::default().is_dark());
                self.surfaces.set_visible_child_name("grid");
            }
        }
        if model.dialog_generation != self.shown_dialog_generation {
            self.shown_dialog_generation = model.dialog_generation;
            if let Some(dialog) = &model.dialog {
                dialogs::present(&self.parent, dialog, self.sender.clone(), app);
            }
        }
    }
}

fn render_status(pane: &CalendarPane, status: CalendarWriteStatus) {
    pane.status_spinner.stop();
    pane.status_spinner.set_visible(false);
    pane.status_label.set_visible(false);
    pane.status_retry.set_visible(false);
    match status {
        CalendarWriteStatus::Idle => pane.status.set_visible(false),
        CalendarWriteStatus::Saving => {
            pane.status.set_visible(true);
            pane.status_spinner.set_visible(true);
            pane.status_spinner.start();
            pane.status_label.set_label(l10n::calendar_saving());
            pane.status_label.set_visible(true);
        }
        CalendarWriteStatus::Saved => {
            pane.status.set_visible(true);
            pane.status_label.set_label(l10n::calendar_saved());
            pane.status_label.set_visible(true);
        }
        CalendarWriteStatus::Failed => {
            pane.status.set_visible(true);
            pane.status_label
                .set_label(l10n::calendar_save_unconfirmed());
            pane.status_label.set_visible(true);
            pane.status_retry.set_visible(true);
        }
    }
}

fn mode_menu(sender: &relm4::Sender<AppInput>) -> gtk::MenuButton {
    let menu = gtk::MenuButton::new();
    menu.update_property(&[AccessibleProperty::Label(l10n::calendar_view_label())]);
    let list = gtk::Box::new(gtk::Orientation::Vertical, 0);
    for index in 0..=5 {
        let mode = CalendarMode::from_index(index);
        let button = gtk::Button::with_label(mode_label(mode));
        button.add_css_class("flat");
        button.update_property(&[AccessibleProperty::Label(mode_label(mode))]);
        let input = sender.clone();
        button.connect_clicked(move |_| input.emit(AppInput::SetCalendarMode(mode)));
        list.append(&button);
    }
    let popover = gtk::Popover::new();
    popover.set_child(Some(&list));
    menu.set_popover(Some(&popover));
    menu
}

fn icon_button(icon: &str, label: &str) -> gtk::Button {
    let button = gtk::Button::from_icon_name(icon);
    button.set_tooltip_text(Some(label));
    button.update_property(&[AccessibleProperty::Label(label)]);
    button
}

fn mode_label(mode: CalendarMode) -> &'static str {
    match mode {
        CalendarMode::Day => l10n::calendar_view_day(),
        CalendarMode::ThreeDay => l10n::calendar_view_three_day(),
        CalendarMode::WorkWeek => l10n::calendar_view_work_week(),
        CalendarMode::Week => l10n::calendar_view_week(),
        CalendarMode::Month => l10n::calendar_view_month(),
        CalendarMode::Agenda => l10n::calendar_view_agenda(),
    }
}

fn previous_label(mode: CalendarMode) -> String {
    match mode {
        CalendarMode::Day => l10n::calendar_prev_day().to_owned(),
        CalendarMode::ThreeDay => l10n::calendar_prev_days(3),
        CalendarMode::Month => l10n::calendar_prev_month().to_owned(),
        _ => l10n::calendar_prev_week().to_owned(),
    }
}

fn next_label(mode: CalendarMode) -> String {
    match mode {
        CalendarMode::Day => l10n::calendar_next_day().to_owned(),
        CalendarMode::ThreeDay => l10n::calendar_next_days(3),
        CalendarMode::Month => l10n::calendar_next_month().to_owned(),
        _ => l10n::calendar_next_week().to_owned(),
    }
}
