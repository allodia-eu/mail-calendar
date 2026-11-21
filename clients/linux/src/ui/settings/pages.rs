//! Settings pages whose state maps directly to shared core setters.
//!
//! General lives in its own module; it grew the appearance chooser and this file was at the
//! 500-line limit.

use adw::prelude::*;
use mailcal_bindings::{
    Intent, PreferenceIntent, QuoteStyleKind, SwipeActionKind, SwipeDirection, ViewMode, WeekStart,
};

use super::{PageContext, choice, dialog_box, group, page_box};

// A child of this module rather than of `settings.rs`, which is at its line limit.
#[path = "default_calendar.rs"]
mod default_calendar;
#[path = "send_account.rs"]
mod send_account;
use crate::{l10n, ui::preference};

pub(super) fn calendar(ctx: &PageContext) -> gtk::Box {
    let content = page_box(l10n::settings_category_calendar());
    content.append(&default_calendar::default_calendar(ctx));
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
        let start = if picker.selected() == 0 {
            WeekStart::Monday
        } else {
            WeekStart::Sunday
        };
        preference::set(&app, PreferenceIntent::SetWeekStart { start });
    });
    week.add(&row);
    content.append(&week);

    let horizon = group(
        l10n::settings_horizon_heading(),
        l10n::settings_horizon_description(),
    );
    // The core's choices hold the horizon in force, wherever a pinch left it.
    let hours = display.visible_hours_choices;
    let labels = hours
        .iter()
        .map(|hours| l10n::settings_horizon_hours(&hours.to_string()))
        .collect::<Vec<_>>();
    let label_refs = labels.iter().map(String::as_str).collect::<Vec<_>>();
    let selected = hours
        .iter()
        .position(|hours| *hours == display.visible_hours)
        .and_then(|index| u32::try_from(index).ok())
        .unwrap_or_default();
    let (row, picker) = choice(l10n::settings_horizon_heading(), &label_refs, selected);
    let app = ctx.app.clone();
    picker.connect_selected_notify(move |picker| {
        if let Some(&hours) = hours.get(picker.selected() as usize) {
            preference::set(&app, PreferenceIntent::SetCalendarVisibleHours { hours });
        }
    });
    horizon.add(&row);
    content.append(&horizon);
    content
}

pub(super) fn reading(ctx: &PageContext) -> gtk::Box {
    let content = page_box(l10n::settings_category_reading());
    let grouping = group(
        l10n::settings_grouping_heading(),
        l10n::settings_grouping_description(),
    );
    // Threaded first, the default, as every platform lists it.
    let selected = match ctx.app.view_mode() {
        ViewMode::Threaded => 0,
        ViewMode::Flat => 1,
    };
    let (row, picker) = choice(
        l10n::settings_grouping_heading(),
        &[
            l10n::settings_grouping_threaded(),
            l10n::settings_grouping_flat(),
        ],
        selected,
    );
    let app = ctx.app.clone();
    picker.connect_selected_notify(move |picker| {
        app.dispatch(Intent::SetViewMode {
            mode: if picker.selected() == 0 {
                ViewMode::Threaded
            } else {
                ViewMode::Flat
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
    content.append(&super::dark_messages::section(ctx));
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
        let style = if picker.selected() == 0 {
            QuoteStyleKind::Indented
        } else {
            QuoteStyleKind::LineAndHeader
        };
        preference::set(&app, PreferenceIntent::SetQuoteStyle { style });
    });
    quote.add(&row);
    let per_message = adw::SwitchRow::builder()
        .title(l10n::settings_quote_per_message_heading())
        .subtitle(l10n::settings_quote_per_message_description())
        .active(settings.per_message)
        .use_markup(false)
        .build();
    let app = ctx.app.clone();
    per_message.connect_active_notify(move |row| {
        let per_message = row.is_active();
        preference::set(
            &app,
            PreferenceIntent::SetQuoteStylePerMessage { per_message },
        );
    });
    quote.add(&per_message);
    content.append(&quote);

    let send = group(
        l10n::settings_send_account_heading(),
        l10n::settings_send_account_description(),
    );
    send.add(&send_account::row(ctx));
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
    row.connect_active_notify(move |row| {
        let enabled = row.is_active();
        preference::set(&app, PreferenceIntent::SetAnalyticsConsent { enabled });
    });
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
    // Absent while the core offers no writing assistant (`docs/ai.md` rule 4).
    content.append(&super::ai_endpoint::section(ctx));
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
        let direction = if left {
            SwipeDirection::Left
        } else {
            SwipeDirection::Right
        };
        preference::set(&app, PreferenceIntent::SetSwipeAction { direction, action });
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
