//! Settings pages whose state maps directly to shared core setters.
//!
//! General lives in its own module; it grew the appearance chooser and this file was at the
//! 500-line limit.

use adw::prelude::*;
use mailcal_bindings::{
    CalendarRow, Intent, QuoteStyleKind, SwipeActionKind, SwipeDirection, ViewMode, WeekStart,
};

use super::{PageContext, choice, dialog_box, group, page_box};
use crate::l10n;

pub(super) fn calendar(ctx: &PageContext) -> gtk::Box {
    let content = page_box(l10n::settings_category_calendar());
    content.append(&default_calendar(ctx));
    let display = ctx.app.display_settings();
    let week = group(
        l10n::settings_week_start_heading(),
        l10n::settings_week_start_description(),
    );
    let selected = match display.week_start {
        WeekStart::Monday => 0,
        WeekStart::Sunday => 1,
    };
    let (row, picker) = choice(
        l10n::settings_week_start_heading(),
        &[
            l10n::settings_week_start_monday(),
            l10n::settings_week_start_sunday(),
        ],
        selected,
    );
    let app = ctx.app.clone();
    picker.connect_selected_notify(move |picker| {
        app.set_week_start(if picker.selected() == 0 {
            WeekStart::Monday
        } else {
            WeekStart::Sunday
        });
    });
    week.add(&row);
    content.append(&week);

    let horizon = group(
        l10n::settings_horizon_heading(),
        l10n::settings_horizon_description(),
    );
    let hours = [6_u8, 8, 10, 12, 16, 24];
    let labels = hours
        .iter()
        .map(|hours| l10n::settings_horizon_hours(&hours.to_string()))
        .collect::<Vec<_>>();
    let label_refs = labels.iter().map(String::as_str).collect::<Vec<_>>();
    let selected = hours
        .iter()
        .position(|hours| *hours == display.visible_hours)
        .and_then(|index| u32::try_from(index).ok())
        .unwrap_or(3);
    let (row, picker) = choice(l10n::settings_horizon_heading(), &label_refs, selected);
    let app = ctx.app.clone();
    picker.connect_selected_notify(move |picker| {
        if let Some(hours) = hours.get(picker.selected() as usize) {
            app.set_calendar_visible_hours(*hours);
        }
    });
    horizon.add(&row);
    content.append(&horizon);
    content
}

fn default_calendar(ctx: &PageContext) -> adw::PreferencesGroup {
    let group = group(
        l10n::settings_default_calendar_heading(),
        l10n::settings_default_calendar_description(),
    );
    let calendars = ctx
        .app
        .calendars()
        .into_iter()
        .filter(|calendar| calendar.can_write)
        .collect::<Vec<_>>();
    if calendars.is_empty() {
        group.add(
            &adw::ActionRow::builder()
                .title(l10n::settings_default_calendar_none())
                .use_markup(false)
                .build(),
        );
        return group;
    }
    let snapshot = ctx.app.mailbox_list();
    let mut calendars_by_account = std::collections::BTreeMap::<&str, Vec<&CalendarRow>>::new();
    for calendar in &calendars {
        calendars_by_account
            .entry(&calendar.account)
            .or_default()
            .push(calendar);
    }
    let multiple_accounts = calendars_by_account.len() > 1;
    let mut check_group: Option<gtk::CheckButton> = None;
    for (account_id, calendars) in calendars_by_account {
        if multiple_accounts {
            let account = snapshot
                .accounts
                .iter()
                .find(|account| account.id == account_id)
                .map_or(account_id, |account| account.email.as_str());
            let heading = adw::ActionRow::builder()
                .title(account)
                .sensitive(false)
                .use_markup(false)
                .build();
            heading.add_css_class("property");
            group.add(&heading);
        }
        for calendar in calendars {
            group.add(&default_calendar_row(ctx, calendar, &mut check_group));
        }
    }
    group
}

fn default_calendar_row(
    ctx: &PageContext,
    calendar: &CalendarRow,
    check_group: &mut Option<gtk::CheckButton>,
) -> adw::ActionRow {
    let row = adw::ActionRow::builder()
        .title(&calendar.name)
        .use_markup(false)
        .build();
    let selected = gtk::CheckButton::builder()
        .active(calendar.is_default)
        .valign(gtk::Align::Center)
        .build();
    if let Some(first) = check_group {
        selected.set_group(Some(first));
    } else {
        *check_group = Some(selected.clone());
    }
    let app = ctx.app.clone();
    let account = calendar.account.clone();
    let id = calendar.id.clone();
    selected.connect_toggled(move |choice| {
        if choice.is_active() {
            app.set_default_calendar(Some(account.clone()), Some(id.clone()));
        }
    });
    row.add_prefix(&selected);
    row.set_activatable_widget(Some(&selected));
    row
}

pub(super) fn reading(ctx: &PageContext) -> gtk::Box {
    let content = page_box(l10n::settings_category_reading());
    let grouping = group(
        l10n::settings_grouping_heading(),
        l10n::settings_grouping_description(),
    );
    let selected = match ctx.app.view_mode() {
        ViewMode::Flat => 0,
        ViewMode::Threaded => 1,
    };
    let (row, picker) = choice(
        l10n::settings_grouping_heading(),
        &[
            l10n::settings_grouping_flat(),
            l10n::settings_grouping_threaded(),
        ],
        selected,
    );
    let app = ctx.app.clone();
    picker.connect_selected_notify(move |picker| {
        app.dispatch(Intent::SetViewMode {
            mode: if picker.selected() == 0 {
                ViewMode::Flat
            } else {
                ViewMode::Threaded
            },
        });
    });
    grouping.add(&row);
    content.append(&grouping);
    let swipe = group(
        l10n::settings_swipe_heading(),
        l10n::settings_swipe_description(),
    );
    let settings = ctx.app.swipe_settings();
    swipe.add(&swipe_row(
        ctx,
        l10n::settings_swipe_left(),
        &SwipeDirection::Left,
        &settings.left,
    ));
    swipe.add(&swipe_row(
        ctx,
        l10n::settings_swipe_right(),
        &SwipeDirection::Right,
        &settings.right,
    ));
    content.append(&swipe);
    content
}

pub(super) fn composing(ctx: &PageContext) -> gtk::Box {
    let content = page_box(l10n::settings_category_composing());
    let quote = group(
        l10n::quote_style_label(),
        l10n::settings_composing_description(),
    );
    let settings = ctx.app.quote_settings();
    let selected = match settings.style {
        QuoteStyleKind::Indented => 0,
        QuoteStyleKind::LineAndHeader => 1,
    };
    let (row, picker) = choice(
        l10n::quote_style_label(),
        &[
            l10n::quote_style_indented(),
            l10n::quote_style_line_header(),
        ],
        selected,
    );
    let app = ctx.app.clone();
    picker.connect_selected_notify(move |picker| {
        app.set_quote_style(if picker.selected() == 0 {
            QuoteStyleKind::Indented
        } else {
            QuoteStyleKind::LineAndHeader
        });
    });
    quote.add(&row);
    let per_message = adw::SwitchRow::builder()
        .title(l10n::quote_style_label())
        .subtitle(l10n::settings_composing_description())
        .active(settings.per_message)
        .use_markup(false)
        .build();
    let app = ctx.app.clone();
    per_message.connect_active_notify(move |row| app.set_quote_style_per_message(row.is_active()));
    quote.add(&per_message);
    content.append(&quote);

    let send = group(
        l10n::settings_send_account_heading(),
        l10n::settings_send_account_description(),
    );
    let snapshot = ctx.app.mailbox_list();
    let labels = snapshot
        .accounts
        .iter()
        .map(|account| account.email.as_str())
        .collect::<Vec<_>>();
    let stored = ctx.app.default_send_account();
    let selected = snapshot
        .accounts
        .iter()
        .position(|account| Some(&account.id) == stored.as_ref())
        .and_then(|index| u32::try_from(index).ok())
        .unwrap_or(0);
    let (row, picker) = choice(l10n::settings_send_account_heading(), &labels, selected);
    let app = ctx.app.clone();
    let accounts = snapshot.accounts;
    picker.connect_selected_notify(move |picker| {
        app.set_default_send_account(
            accounts
                .get(picker.selected() as usize)
                .map(|account| account.id.clone()),
        );
    });
    send.add(&row);
    content.append(&send);
    content
}

pub(super) fn notifications(ctx: &PageContext) -> gtk::Box {
    let content = page_box(l10n::settings_category_notifications());
    let group = group(
        l10n::settings_notifications_heading(),
        l10n::settings_notifications_description(),
    );
    let row = adw::SwitchRow::builder()
        .title(l10n::settings_notifications_heading())
        .subtitle(l10n::settings_notifications_description())
        .active(ctx.preferences.notifications_enabled())
        .use_markup(false)
        .build();
    let preferences = ctx.preferences.clone();
    row.connect_active_notify(move |row| preferences.set_notifications_enabled(row.is_active()));
    group.add(&row);
    content.append(&group);
    content
}

pub(super) fn privacy(ctx: &PageContext) -> gtk::Box {
    let content = page_box(l10n::settings_category_privacy());
    let group = group(
        l10n::settings_analytics_heading(),
        l10n::settings_analytics_description(),
    );
    let row = adw::SwitchRow::builder()
        .title(l10n::settings_analytics_toggle())
        .subtitle(l10n::settings_analytics_description())
        .active(ctx.app.analytics_consent().enabled)
        .use_markup(false)
        .build();
    let app = ctx.app.clone();
    row.connect_active_notify(move |row| app.set_analytics_consent(row.is_active()));
    group.add(&row);
    let preview = adw::ActionRow::builder()
        .title(l10n::welcome_analytics_preview())
        .use_markup(false)
        .build();
    let button = gtk::Button::with_label(l10n::welcome_analytics_preview());
    let app = ctx.app.clone();
    let parent = ctx.window.clone();
    button.connect_clicked(move |_| {
        show_text(
            &parent,
            l10n::welcome_analytics_preview(),
            &app.analytics_payload_preview(),
        );
    });
    preview.add_suffix(&button);
    group.add(&preview);
    content.append(&group);
    content
}

pub(super) fn advanced(ctx: &PageContext) -> gtk::Box {
    let content = page_box(l10n::settings_category_advanced());
    let group = group(
        l10n::action_reset_database(),
        l10n::settings_advanced_reset_description(),
    );
    let row = adw::ActionRow::builder()
        .title(l10n::action_reset_database())
        .subtitle(l10n::settings_advanced_reset_description())
        .use_markup(false)
        .build();
    let button = gtk::Button::with_label(l10n::action_reset_database());
    button.add_css_class("destructive-action");
    let parent = ctx.window.clone();
    let app = ctx.app.clone();
    button.connect_clicked(move |_| confirm_reset(&parent, app.clone()));
    row.add_suffix(&button);
    group.add(&row);
    content.append(&group);
    if let Some(mcp) = super::mcp::section(ctx) {
        content.append(&mcp);
    }
    content
}

fn swipe_row(
    ctx: &PageContext,
    title: &str,
    direction: &SwipeDirection,
    current: &SwipeActionKind,
) -> adw::ActionRow {
    let left = matches!(direction, SwipeDirection::Left);
    let selected = match current {
        SwipeActionKind::Delete => 0,
        SwipeActionKind::Archive => 1,
        SwipeActionKind::Star => 2,
    };
    let (row, picker) = choice(
        title,
        &[
            l10n::swipe_action_delete(),
            l10n::swipe_action_archive(),
            l10n::swipe_action_star(),
        ],
        selected,
    );
    let app = ctx.app.clone();
    picker.connect_selected_notify(move |picker| {
        let action = match picker.selected() {
            1 => SwipeActionKind::Archive,
            2 => SwipeActionKind::Star,
            _ => SwipeActionKind::Delete,
        };
        app.set_swipe_action(
            if left {
                SwipeDirection::Left
            } else {
                SwipeDirection::Right
            },
            action,
        );
    });
    row
}

fn confirm_reset(parent: &gtk::Window, app: std::sync::Arc<mailcal_bindings::MailcalApp>) {
    let dialog = confirm_window(parent, l10n::reset_title(), l10n::reset_message());
    let actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    actions.set_halign(gtk::Align::End);
    let cancel = gtk::Button::with_label(l10n::action_cancel());
    let window = dialog.clone();
    cancel.connect_clicked(move |_| window.close());
    actions.append(&cancel);
    let reset = gtk::Button::with_label(l10n::reset_confirm());
    reset.add_css_class("destructive-action");
    let window = dialog.clone();
    reset.connect_clicked(move |_| {
        app.reset();
        window.close();
    });
    actions.append(&reset);
    dialog
        .child()
        .and_downcast::<gtk::Box>()
        .expect("confirmation content")
        .append(&actions);
    dialog.present();
}

pub(super) fn confirm_window(parent: &gtk::Window, title: &str, message: &str) -> gtk::Window {
    let (dialog, _) = crate::ui::modal::new(parent, title, 440, None);
    let content = dialog_box();
    let body = gtk::Label::new(Some(message));
    body.set_wrap(true);
    body.set_xalign(0.0);
    content.append(&body);
    dialog.set_child(Some(&content));
    dialog
}

pub(super) fn show_text(parent: &gtk::Window, title: &str, text: &str) {
    show_text_window(parent, title, text, false);
}

pub(super) fn show_text_at_end(parent: &gtk::Window, title: &str, text: &str) {
    show_text_window(parent, title, text, true);
}

fn show_text_window(parent: &gtk::Window, title: &str, text: &str, start_at_end: bool) {
    let (dialog, _) = crate::ui::modal::new(parent, title, 680, Some(500));
    let content = dialog_box();
    let view = gtk::TextView::new();
    view.set_editable(false);
    view.set_monospace(true);
    view.buffer().set_text(text);
    let scroll = gtk::ScrolledWindow::new();
    scroll.set_vexpand(true);
    scroll.set_child(Some(&view));
    content.append(&scroll);
    let actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    actions.set_halign(gtk::Align::End);
    if start_at_end {
        let jump = gtk::Button::with_label(l10n::diagnostics_jump_to_end());
        let target = view.clone();
        jump.connect_clicked(move |_| scroll_to_end(&target));
        actions.append(&jump);
        let target = view.clone();
        gtk::glib::idle_add_local_once(move || scroll_to_end(&target));
    }
    let close = gtk::Button::with_label(l10n::action_close());
    let window = dialog.clone();
    close.connect_clicked(move |_| window.close());
    actions.append(&close);
    content.append(&actions);
    dialog.set_child(Some(&content));
    dialog.present();
}

fn scroll_to_end(view: &gtk::TextView) {
    let mut end = view.buffer().end_iter();
    view.scroll_to_iter(&mut end, 0.0, false, 0.0, 0.0);
}
