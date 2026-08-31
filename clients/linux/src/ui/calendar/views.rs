//! Composed agenda and month surfaces (the pinch-sensitive time grid remains drawn).

use std::{cell::RefCell, collections::HashSet};

use adw::prelude::*;
use gtk::accessible::Property as AccessibleProperty;

use super::{
    super::AppInput,
    date::{clock, local_date_time, parse_date},
    model::{CalendarModel, EventIdentity},
    paint,
};
use crate::l10n;

const MONTH_CAPACITY: usize = 4;

pub(super) fn render_agenda(
    list: &gtk::ListBox,
    model: &CalendarModel,
    sender: &relm4::Sender<AppInput>,
) {
    clear_list(list);
    if !model.page.is_materialized {
        list.append(&message_row(l10n::calendar_loading_range()));
        return;
    }
    if model.agenda.events.is_empty() {
        list.append(&message_row(l10n::calendar_no_events()));
        return;
    }
    for event in &model.agenda.events {
        let title = title(&event.title);
        // An agenda row is a list item with no border to dash, so it *prints* the hold instead,
        // and says it in the spoken label too, which is the part that may not vary by surface
        // (`docs/calendar.md` §4).
        let awaiting = paint::is_awaiting(event.participation);
        let when = local_date_time(&event.start, &model.agenda.timezone, model.use_24_hour);
        let subtitle = if awaiting {
            format!("{when} · {}", l10n::a11y_invitation_awaiting_response())
        } else {
            when
        };
        let row = adw::ActionRow::builder()
            .title(&title)
            .subtitle(&subtitle)
            .use_markup(false)
            .build();
        if awaiting {
            // `Description`, not `Label`: the row labels itself from its title through a
            // `labelled-by` relation, and a relation beats an explicit label. Reached through the
            // widget because libadwaita's wrapper does not list `gtk::Accessible` among the
            // interfaces `AdwActionRow` implements, so the extension trait is otherwise out of
            // reach.
            row.upcast_ref::<gtk::Widget>()
                .update_property(&[AccessibleProperty::Description(
                    l10n::a11y_invitation_awaiting_response(),
                )]);
        }
        let open = gtk::Button::from_icon_name("go-next-symbolic");
        open.set_tooltip_text(Some(&title));
        open.update_property(&[AccessibleProperty::Label(&title)]);
        open.set_valign(gtk::Align::Center);
        let input = sender.clone();
        // An agenda row *is* the series; one row per event, not per occurrence; so it names
        // no occurrence and a write from it reaches the whole thing.
        let identity = EventIdentity {
            account: event.account.clone(),
            key: event.key.clone(),
            occurrence: String::new(),
        };
        open.connect_clicked(move |_| {
            input.emit(AppInput::OpenCalendarEvent(identity.clone()));
        });
        row.add_suffix(&open);
        if event.can_write {
            let delete = gtk::Button::from_icon_name("user-trash-symbolic");
            delete.set_tooltip_text(Some(l10n::action_delete_event()));
            delete.update_property(&[AccessibleProperty::Label(l10n::action_delete_event())]);
            delete.add_css_class("flat");
            delete.set_valign(gtk::Align::Center);
            let input = sender.clone();
            let identity = EventIdentity {
                account: event.account.clone(),
                key: event.key.clone(),
                occurrence: String::new(),
            };
            delete.connect_clicked(move |_| {
                input.emit(AppInput::RequestDeleteEvent(identity.clone()));
            });
            row.add_suffix(&delete);
        }
        list.append(&row);
    }
}

pub(super) fn render_month(
    grid: &gtk::Grid,
    model: &CalendarModel,
    sender: &relm4::Sender<AppInput>,
) {
    clear_grid(grid);
    if !model.month.is_materialized {
        let label = gtk::Label::new(Some(l10n::calendar_loading_range()));
        label.set_margin_top(48);
        grid.attach(&label, 0, 0, 7, 1);
        return;
    }
    for (index, cell) in model.month.cells.iter().enumerate() {
        let Ok(column) = i32::try_from(index % 7) else {
            continue;
        };
        let Ok(row) = i32::try_from(index / 7) else {
            continue;
        };
        let day = gtk::Box::new(gtk::Orientation::Vertical, 3);
        day.set_margin_start(4);
        day.set_margin_end(4);
        day.set_margin_top(4);
        day.set_margin_bottom(4);
        day.set_size_request(96, 96);
        if !cell.in_month {
            day.set_opacity(0.55);
        }
        let date_button = gtk::Button::with_label(
            &parse_date(&cell.date)
                .map_or_else(|| cell.date.clone(), |date| date.day().to_string()),
        );
        date_button.add_css_class("flat");
        date_button.set_halign(gtk::Align::Start);
        date_button.update_property(&[AccessibleProperty::Label(&cell.date)]);
        let input = sender.clone();
        let date = cell.date.clone();
        date_button.connect_clicked(move |_| input.emit(AppInput::ShowCalendarDay(date.clone())));
        day.append(&date_button);

        let visible = month_visible_count(cell.chips.len(), MONTH_CAPACITY);
        for chip in cell.chips.iter().take(visible) {
            let title = title(&chip.title);
            let label = if chip.all_day {
                title.clone()
            } else {
                format!("● {} {title}", clock(chip.start_minutes, model.use_24_hour))
            };
            let button = gtk::Button::with_label(&label);
            button.add_css_class("flat");
            button.set_halign(gtk::Align::Fill);
            button.set_tooltip_text(Some(&title));
            // A chip is a few points tall, so the hatch is what survives and the dashes ride
            // beside it: and the disclosure is spoken whatever the chip shows. `Description`
            // again, for the reason the agenda row gives.
            let awaiting = paint::is_awaiting(chip.participation);
            if awaiting {
                button.update_property(&[AccessibleProperty::Description(
                    l10n::a11y_invitation_awaiting_response(),
                )]);
            }
            apply_month_style(&button, model, &chip.account, &chip.calendar, awaiting);
            let input = sender.clone();
            let identity = EventIdentity {
                account: chip.account.clone(),
                key: chip.event.clone(),
                occurrence: chip.occurrence_start.clone(),
            };
            button.connect_clicked(move |_| {
                input.emit(AppInput::OpenCalendarEvent(identity.clone()));
            });
            day.append(&button);
        }
        if visible < cell.chips.len() {
            let hidden = i64::try_from(cell.chips.len() - visible).unwrap_or(i64::MAX);
            let more = gtk::Button::with_label(&l10n::calendar_all_day_more(hidden));
            more.add_css_class("flat");
            let input = sender.clone();
            let date = cell.date.clone();
            more.connect_clicked(move |_| input.emit(AppInput::ShowCalendarDay(date.clone())));
            day.append(&more);
        }
        let frame = gtk::Frame::new(None);
        frame.set_child(Some(&day));
        frame.set_hexpand(true);
        frame.set_vexpand(true);
        grid.attach(&frame, column, row, 1, 1);
    }
}

fn apply_month_style(
    button: &gtk::Button,
    model: &CalendarModel,
    account: &str,
    calendar: &str,
    awaiting: bool,
) {
    let Some(row) = model
        .month
        .calendars
        .iter()
        .find(|row| row.account == account && row.id == calendar)
    else {
        return;
    };
    let swatch = if adw::StyleManager::default().is_dark() {
        &row.color.dark
    } else {
        &row.color.light
    };
    let (Some(background), Some(text), Some(border)) = (
        css_hex(&swatch.background),
        css_hex(&swatch.text),
        css_hex(&swatch.border),
    ) else {
        return;
    };
    let class = format!(
        "calendar-chip-{}-{}-{}{}",
        &background[1..],
        &text[1..],
        &border[1..],
        if awaiting { "-hold" } else { "" }
    );
    button.add_css_class(&class);
    install_chip_style(&class, background, text, border, awaiting);
}

thread_local! {
    static CHIP_STYLES: RefCell<HashSet<String>> = RefCell::new(HashSet::new());
}

fn install_chip_style(class: &str, background: &str, text: &str, border: &str, awaiting: bool) {
    CHIP_STYLES.with(|styles| {
        if !styles.borrow_mut().insert(class.to_owned()) {
            return;
        }
        let fill = if awaiting {
            faded(background)
        } else {
            background.to_owned()
        };
        let treatment = if awaiting {
            paint::hold_css(&fill, border)
        } else {
            format!("background: {fill}; border-color: {border};")
        };
        let provider = gtk::CssProvider::new();
        provider.load_from_string(&format!(".{class} {{ {treatment} color: {text}; }}"));
        if let Some(display) = gtk::gdk::Display::default() {
            gtk::style_context_add_provider_for_display(
                &display,
                &provider,
                gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
            );
        }
    });
}

/// A `#rrggbb` swatch as the CSS colour a hold's fill is drawn in: the same fade the Cairo
/// surfaces apply, expressed the one way GTK CSS can express it.
fn faded(background: &str) -> String {
    let component = |range: std::ops::Range<usize>| {
        background
            .get(range)
            .and_then(|part| u8::from_str_radix(part, 16).ok())
            .unwrap_or(0)
    };
    format!(
        "rgba({}, {}, {}, {})",
        component(1..3),
        component(3..5),
        component(5..7),
        paint::HOLD_FILL_ALPHA
    )
}

fn css_hex(value: &str) -> Option<&str> {
    (value.len() == 7
        && value.starts_with('#')
        && value[1..].bytes().all(|byte| byte.is_ascii_hexdigit()))
    .then_some(value)
}

pub(super) const fn month_visible_count(total: usize, capacity: usize) -> usize {
    if total <= capacity {
        total
    } else {
        capacity.saturating_sub(1)
    }
}

fn title(value: &str) -> String {
    if value.trim().is_empty() {
        l10n::event_no_title().to_owned()
    } else {
        value.to_owned()
    }
}

fn message_row(message: &str) -> adw::ActionRow {
    adw::ActionRow::builder()
        .title(message)
        .use_markup(false)
        .build()
}

fn clear_list(list: &gtk::ListBox) {
    while let Some(child) = list.first_child() {
        list.remove(&child);
    }
}

fn clear_grid(grid: &gtk::Grid) {
    while let Some(child) = grid.first_child() {
        grid.remove(&child);
    }
}

#[cfg(test)]
mod tests {
    use super::month_visible_count;

    #[test]
    fn month_overflow_row_represents_more_than_it_displaces() {
        assert_eq!(month_visible_count(4, 4), 4);
        assert_eq!(month_visible_count(5, 4), 3);
        assert_eq!(5 - month_visible_count(5, 4), 2);
    }
}
