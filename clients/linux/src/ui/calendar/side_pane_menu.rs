//! A calendar row's colour: the round check it is drawn with, and the context menu that changes
//! it (`docs/calendar-pane.md`, rules 8 and 9), in the folder menu's idiom: right click, a long
//! press, or the Menu key and Shift+F10 on a focused row.

use std::{f64::consts::TAU, rc::Rc};

use adw::prelude::*;
use gtk::accessible::Property as AccessibleProperty;
use mailcal_bindings::{CalendarRow, calendar_palette};

use super::side_pane::PaneInput;
use crate::{
    l10n,
    ui::{AppInput, folder_menu::on_menu_request},
};

/// The check's diameter.
const CHECK: i32 = 16;
/// A palette swatch's diameter.
const SWATCH: i32 = 30;
/// What a colour that is not `#rrggbb` is drawn in.
const GREY: (f64, f64, f64) = (0.35, 0.35, 0.35);

/// The round check: filled with the calendar's colour and ticked while it shows, a ring in that
/// colour while it is hidden. The row it sits in says which to a screen reader.
pub(super) fn check(hex: &str, shown: bool) -> gtk::DrawingArea {
    let area = gtk::DrawingArea::builder()
        .accessible_role(gtk::AccessibleRole::Presentation)
        .content_width(CHECK)
        .content_height(CHECK)
        .valign(gtk::Align::Center)
        .build();
    let (red, green, blue) = rgb(hex).unwrap_or(GREY);
    area.set_draw_func(move |_, context, width, height| {
        let (x, y) = (f64::from(width) / 2.0, f64::from(height) / 2.0);
        let radius = f64::from(width.min(height)) / 2.0;
        context.set_source_rgb(red, green, blue);
        if !shown {
            context.set_line_width(2.0);
            context.arc(x, y, radius - 1.0, 0.0, TAU);
            let _ = context.stroke();
            return;
        }
        context.arc(x, y, radius, 0.0, TAU);
        let _ = context.fill();
        let (w, h) = (f64::from(width), f64::from(height));
        context.set_source_rgb(1.0, 1.0, 1.0);
        context.set_line_width(2.0);
        context.set_line_cap(gtk::cairo::LineCap::Round);
        context.set_line_join(gtk::cairo::LineJoin::Round);
        context.move_to(w * 0.28, h * 0.52);
        context.line_to(w * 0.44, h * 0.68);
        context.line_to(w * 0.72, h * 0.36);
        let _ = context.stroke();
    });
    area
}

/// Gives `row` its menu: the colour, which opens the palette, and the server's colour.
pub(super) fn attach(
    row: &adw::ActionRow,
    calendar: &CalendarRow,
    sender: &relm4::Sender<AppInput>,
) {
    let pages = gtk::Stack::new();
    pages.set_transition_type(gtk::StackTransitionType::None);
    let menu = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let popover = gtk::Popover::new();
    popover.set_has_arrow(false);
    popover.set_child(Some(&pages));
    popover.set_parent(row);

    let colour = gtk::Button::with_label(l10n::calendar_pane_color());
    colour.add_css_class("flat");
    let palette_page = pages.clone();
    colour.connect_clicked(move |_| palette_page.set_visible_child_name("palette"));
    menu.append(&colour);
    let reset = gtk::Button::with_label(l10n::calendar_color_reset());
    reset.add_css_class("flat");
    let choose = chooser(sender, calendar, &popover);
    let reset_choice = choose.clone();
    reset.connect_clicked(move |_| reset_choice(None));
    menu.append(&reset);
    pages.add_named(&padded(&menu), Some("menu"));
    pages.add_named(&padded(&palette(&choose)), Some("palette"));
    let first_page = pages.clone();
    popover.connect_closed(move |_| first_page.set_visible_child_name("menu"));

    let (at_pointer, from_keyboard) = (popover.clone(), popover.clone());
    on_menu_request(
        row,
        move |x, y| {
            // Truncating a pointer position to a whole pixel is what a pointing rectangle is.
            #[allow(clippy::cast_possible_truncation)]
            at_pointer.set_pointing_to(Some(&gtk::gdk::Rectangle::new(x as i32, y as i32, 1, 1)));
            at_pointer.popup();
        },
        move || {
            from_keyboard.set_pointing_to(None);
            from_keyboard.popup();
        },
    );
    row.connect_destroy(move |_| {
        popover.popdown();
        popover.unparent();
    });
}

/// Sets the calendar's colour, `None` for the server's, and closes the menu.
fn chooser(
    sender: &relm4::Sender<AppInput>,
    calendar: &CalendarRow,
    popover: &gtk::Popover,
) -> Rc<dyn Fn(Option<String>)> {
    let (input, account, id, popover) = (
        sender.clone(),
        calendar.account.clone(),
        calendar.id.clone(),
        popover.clone(),
    );
    Rc::new(move |hex| {
        popover.popdown();
        input.emit(AppInput::CalendarPane(PaneInput::Colour {
            account: account.clone(),
            calendar: id.clone(),
            hex,
        }));
    })
}

fn palette(choose: &Rc<dyn Fn(Option<String>)>) -> gtk::FlowBox {
    let choices = gtk::FlowBox::new();
    choices.set_selection_mode(gtk::SelectionMode::None);
    choices.set_max_children_per_line(5);
    choices.set_min_children_per_line(5);
    choices.set_column_spacing(8);
    choices.set_row_spacing(8);
    for hex in calendar_palette() {
        let button = gtk::Button::new();
        button.add_css_class("flat");
        button.update_property(&[AccessibleProperty::Label(&hex)]);
        button.set_widget_name(&format!("calendar-colour-{}", hex.trim_start_matches('#')));
        button.set_child(Some(&swatch(&hex)));
        let choose = Rc::clone(choose);
        button.connect_clicked(move |_| choose(Some(hex.clone())));
        choices.insert(&button, -1);
    }
    choices
}

fn padded(child: &impl IsA<gtk::Widget>) -> gtk::Box {
    let page = gtk::Box::new(gtk::Orientation::Vertical, 0);
    page.set_margin_top(6);
    page.set_margin_bottom(6);
    page.set_margin_start(6);
    page.set_margin_end(6);
    page.append(child);
    page
}

fn swatch(hex: &str) -> gtk::DrawingArea {
    let area = gtk::DrawingArea::builder()
        .accessible_role(gtk::AccessibleRole::Presentation)
        .content_width(SWATCH)
        .content_height(SWATCH)
        .build();
    let (red, green, blue) = rgb(hex).unwrap_or(GREY);
    area.set_draw_func(move |_, context, width, height| {
        context.set_source_rgb(red, green, blue);
        context.arc(
            f64::from(width) / 2.0,
            f64::from(height) / 2.0,
            f64::from(width.min(height)) / 2.0,
            0.0,
            TAU,
        );
        let _ = context.fill();
    });
    area
}

/// A `#rrggbb` colour as cairo's components; anything else is not drawn in.
pub(super) fn rgb(hex: &str) -> Option<(f64, f64, f64)> {
    let value = hex.strip_prefix('#')?;
    (value.len() == 6).then_some(())?;
    let channel = |start| u8::from_str_radix(value.get(start..start + 2)?, 16).ok();
    Some((
        f64::from(channel(0)?) / 255.0,
        f64::from(channel(2)?) / 255.0,
        f64::from(channel(4)?) / 255.0,
    ))
}

#[cfg(test)]
mod tests {
    use super::rgb;

    #[test]
    fn only_six_digit_hex_colors_reach_the_check() {
        assert_eq!(rgb("#ff8000"), Some((1.0, 128.0 / 255.0, 0.0)));
        assert_eq!(rgb("orange"), None);
        assert_eq!(rgb("#fff"), None);
        assert_eq!(rgb("#ééé"), None);
    }
}
