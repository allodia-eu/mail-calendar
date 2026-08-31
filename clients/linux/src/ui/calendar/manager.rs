//! Calendar visibility and colour manager, grouped by owning account.

use std::{cell::RefCell, collections::BTreeMap, rc::Rc, sync::Arc};

use adw::prelude::*;
use gtk::accessible::Property as AccessibleProperty;
use mailcal_bindings::{
    AccountRow, CalendarColor, CalendarRow, MailcalApp, Swatch, calendar_palette,
};

use crate::l10n;

#[derive(Debug, Default)]
pub(super) struct CalendarManager {
    window: Option<gtk::Window>,
    rendered_generation: u64,
}

impl CalendarManager {
    pub(super) fn render(
        &mut self,
        generation: u64,
        parent: &adw::ApplicationWindow,
        app: Option<&Arc<MailcalApp>>,
        accounts: &[AccountRow],
    ) {
        if generation == 0 || generation == self.rendered_generation {
            return;
        }
        let Some(app) = app else {
            return;
        };
        if let Some(window) = self.window.take() {
            window.close();
        }
        let (window, header) =
            crate::ui::modal::new(parent, l10n::calendar_manage(), 560, Some(640));
        window.set_modal(false);
        window.set_child(Some(&content(&window, &header, app, accounts)));
        window.present();
        self.window = Some(window);
        self.rendered_generation = generation;
    }
}

fn content(
    window: &gtk::Window,
    header: &adw::HeaderBar,
    app: &Arc<MailcalApp>,
    accounts: &[AccountRow],
) -> gtk::Box {
    let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let done = gtk::Button::with_label(l10n::action_done());
    let dialog = window.clone();
    done.connect_clicked(move |_| dialog.close());
    header.pack_end(&done);

    let calendars = app.calendars();
    if calendars.is_empty() {
        let empty = gtk::Label::new(Some(l10n::calendar_manage_empty()));
        empty.set_vexpand(true);
        empty.add_css_class("dim-label");
        root.append(&empty);
        return root;
    }

    let labels = accounts
        .iter()
        .map(|account| (account.id.as_str(), account.email.as_str()))
        .collect::<BTreeMap<_, _>>();
    let mut grouped = BTreeMap::<&str, Vec<&CalendarRow>>::new();
    for calendar in &calendars {
        grouped.entry(&calendar.account).or_default().push(calendar);
    }
    let list = gtk::Box::new(gtk::Orientation::Vertical, 16);
    list.set_margin_start(18);
    list.set_margin_end(18);
    list.set_margin_top(18);
    list.set_margin_bottom(18);
    for (account, calendars) in grouped {
        let group = adw::PreferencesGroup::builder()
            .title(labels.get(account).copied().unwrap_or(account))
            .build();
        for calendar in calendars {
            group.add(&calendar_row(Arc::clone(app), calendar));
        }
        list.append(&group);
    }
    let scroll = gtk::ScrolledWindow::new();
    scroll.set_vexpand(true);
    scroll.set_child(Some(&list));
    root.append(&scroll);
    root
}

fn calendar_row(app: Arc<MailcalApp>, calendar: &CalendarRow) -> adw::ActionRow {
    let row = adw::ActionRow::builder()
        .title(&calendar.name)
        .use_markup(false)
        .build();
    let color = Rc::new(RefCell::new(copy_color(&calendar.color)));
    let color_button = gtk::MenuButton::new();
    let color_label = l10n::calendar_pick_color(&calendar.name);
    color_button.update_property(&[AccessibleProperty::Label(&color_label)]);
    let swatch = resolved_color_swatch(Rc::clone(&color), 24);
    color_button.set_child(Some(&swatch));
    color_button.set_popover(Some(&color_popover(
        Arc::clone(&app),
        calendar,
        color,
        &swatch,
    )));
    row.add_prefix(&color_button);

    let visible = gtk::Switch::builder()
        .active(calendar.visible)
        .valign(gtk::Align::Center)
        .build();
    visible.update_property(&[AccessibleProperty::Label(&calendar.name)]);
    let account = calendar.account.clone();
    let id = calendar.id.clone();
    visible.connect_active_notify(move |control| {
        app.set_calendar_visible(account.clone(), id.clone(), control.is_active());
    });
    row.add_suffix(&visible);
    row.set_activatable_widget(Some(&visible));
    row
}

fn color_popover(
    app: Arc<MailcalApp>,
    calendar: &CalendarRow,
    color: Rc<RefCell<CalendarColor>>,
    swatch: &gtk::DrawingArea,
) -> gtk::Popover {
    let choices = gtk::FlowBox::new();
    choices.set_max_children_per_line(5);
    choices.set_min_children_per_line(5);
    choices.set_column_spacing(8);
    choices.set_row_spacing(8);
    for hex in calendar_palette() {
        let button = gtk::Button::new();
        button.update_property(&[AccessibleProperty::Label(&hex)]);
        button.set_child(Some(&palette_swatch(&hex, 30)));
        let app = Arc::clone(&app);
        let account = calendar.account.clone();
        let id = calendar.id.clone();
        let current = Rc::clone(&color);
        let target = swatch.clone();
        button.connect_clicked(move |_| {
            app.set_calendar_color(account.clone(), id.clone(), Some(hex.clone()));
            update_resolved_color(&app, &account, &id, &current, &target);
        });
        choices.insert(&button, -1);
    }
    let content = gtk::Box::new(gtk::Orientation::Vertical, 10);
    content.set_margin_start(10);
    content.set_margin_end(10);
    content.set_margin_top(10);
    content.set_margin_bottom(10);
    content.append(&choices);
    let reset = gtk::Button::with_label(l10n::calendar_color_reset());
    let account = calendar.account.clone();
    let id = calendar.id.clone();
    let current = color;
    let target = swatch.clone();
    reset.connect_clicked(move |_| {
        app.set_calendar_color(account.clone(), id.clone(), None);
        if let Some(resolved) = app
            .calendars()
            .into_iter()
            .find(|row| row.account == account && row.id == id)
        {
            *current.borrow_mut() = resolved.color;
            target.queue_draw();
        }
    });
    content.append(&reset);
    let popover = gtk::Popover::new();
    popover.set_child(Some(&content));
    popover
}

fn update_resolved_color(
    app: &MailcalApp,
    account: &str,
    id: &str,
    current: &RefCell<CalendarColor>,
    target: &gtk::DrawingArea,
) {
    if let Some(resolved) = app
        .calendars()
        .into_iter()
        .find(|row| row.account == account && row.id == id)
    {
        *current.borrow_mut() = resolved.color;
        target.queue_draw();
    }
}

fn copy_color(color: &CalendarColor) -> CalendarColor {
    CalendarColor {
        hex: color.hex.clone(),
        light: copy_swatch(&color.light),
        dark: copy_swatch(&color.dark),
    }
}

fn copy_swatch(swatch: &Swatch) -> Swatch {
    Swatch {
        background: swatch.background.clone(),
        text: swatch.text.clone(),
        border: swatch.border.clone(),
    }
}

fn resolved_swatch(color: &CalendarColor, dark: bool) -> (&str, &str) {
    let swatch = if dark { &color.dark } else { &color.light };
    (&swatch.background, &swatch.border)
}

#[allow(clippy::cast_precision_loss)]
fn resolved_color_swatch(color: Rc<RefCell<CalendarColor>>, size: i32) -> gtk::DrawingArea {
    let area = gtk::DrawingArea::new();
    area.set_content_width(size);
    area.set_content_height(size);
    area.set_draw_func(move |_, context, width, height| {
        let color = color.borrow();
        let (background, border) = resolved_swatch(&color, adw::StyleManager::default().is_dark());
        let (red, green, blue) = rgb(background).unwrap_or((0.35, 0.35, 0.35));
        let edge = rgb(border).unwrap_or((0.2, 0.2, 0.2));
        context.set_source_rgb(red, green, blue);
        let radius = f64::from(width.min(height)) / 2.0 - 1.0;
        context.arc(
            f64::from(width) / 2.0,
            f64::from(height) / 2.0,
            radius,
            0.0,
            std::f64::consts::TAU,
        );
        let _ = context.fill_preserve();
        context.set_source_rgb(edge.0, edge.1, edge.2);
        context.set_line_width(2.0);
        let _ = context.stroke();
    });
    let redraw = area.downgrade();
    adw::StyleManager::default().connect_dark_notify(move |_| {
        if let Some(redraw) = redraw.upgrade() {
            redraw.queue_draw();
        }
    });
    area
}

fn palette_swatch(hex: &str, size: i32) -> gtk::DrawingArea {
    let area = gtk::DrawingArea::new();
    area.set_content_width(size);
    area.set_content_height(size);
    let color = hex.to_owned();
    area.set_draw_func(move |_, context, width, height| {
        let (red, green, blue) = rgb(&color).unwrap_or((0.35, 0.35, 0.35));
        context.set_source_rgb(red, green, blue);
        context.arc(
            f64::from(width) / 2.0,
            f64::from(height) / 2.0,
            f64::from(width.min(height)) / 2.0,
            0.0,
            std::f64::consts::TAU,
        );
        let _ = context.fill();
    });
    area
}

fn rgb(hex: &str) -> Option<(f64, f64, f64)> {
    let value = hex.strip_prefix('#')?;
    (value.len() == 6).then_some(())?;
    let channel = |start| u8::from_str_radix(&value[start..start + 2], 16).ok();
    Some((
        f64::from(channel(0)?) / 255.0,
        f64::from(channel(2)?) / 255.0,
        f64::from(channel(4)?) / 255.0,
    ))
}

#[cfg(test)]
mod tests {
    use mailcal_bindings::{CalendarColor, Swatch};

    use super::{resolved_swatch, rgb};

    #[test]
    fn only_six_digit_hex_colors_reach_the_swatch() {
        assert_eq!(rgb("#ff8000"), Some((1.0, 128.0 / 255.0, 0.0)));
        assert_eq!(rgb("orange"), None);
        assert_eq!(rgb("#fff"), None);
    }

    #[test]
    fn the_manager_uses_the_active_resolved_fill_and_border() {
        let color = CalendarColor {
            hex: "#bright0".to_owned(),
            light: Swatch {
                background: "#111111".to_owned(),
                text: "#ffffff".to_owned(),
                border: "#222222".to_owned(),
            },
            dark: Swatch {
                background: "#333333".to_owned(),
                text: "#ffffff".to_owned(),
                border: "#444444".to_owned(),
            },
        };

        assert_eq!(resolved_swatch(&color, false), ("#111111", "#222222"));
        assert_eq!(resolved_swatch(&color, true), ("#333333", "#444444"));
    }
}
