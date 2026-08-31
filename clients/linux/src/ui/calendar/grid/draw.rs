//! Cairo paint pass. It only multiplies the scene's unit-free geometry.

use gtk::cairo;

use super::scene::{GUTTER, GridScene, HEADING_HEIGHT, LANE_HEIGHT, pixels};
use crate::{
    l10n,
    ui::calendar::{
        date::{clock, now_in},
        paint::{self, Rect, Rgb, set_source},
    },
};

pub(super) fn draw(scene: &GridScene, context: &cairo::Context, width: f64) {
    let background = theme_color(scene.dark, false);
    let foreground = theme_color(scene.dark, true);
    let line = if scene.dark {
        Rgb::new(0.28, 0.28, 0.28)
    } else {
        Rgb::new(0.86, 0.86, 0.86)
    };
    set_source(context, background);
    let _ = context.paint();
    if !scene.is_materialized {
        set_source(context, foreground);
        select_font(context, 15.0, cairo::FontWeight::Normal);
        context.move_to(GUTTER + 24.0, HEADING_HEIGHT + 48.0);
        let _ = context.show_text(l10n::calendar_loading_range());
        return;
    }
    let geometry = scene.geometry(width);
    draw_lines(scene, context, width, geometry.day_width, line);
    draw_labels(scene, context, geometry.day_width, foreground);
    draw_bands(scene, context, geometry.day_width, foreground);
    draw_events(scene, context, geometry.day_width);
    draw_now(scene, context, width, geometry.day_width);
    draw_create(scene, context, width, geometry.day_width);
}

fn draw_lines(scene: &GridScene, context: &cairo::Context, width: f64, day_width: f64, line: Rgb) {
    set_source(context, line);
    context.set_line_width(1.0);
    for index in 0..=scene.days.len() {
        let x = GUTTER + pixels(index) * day_width;
        context.move_to(x, 0.0);
        context.line_to(x, scene.height());
    }
    for hour in 0..=24 {
        let y = scene.content_top() + f64::from(hour) * scene.hour_height;
        context.move_to(GUTTER, y);
        context.line_to(width, y);
    }
    let _ = context.stroke();
}

fn draw_labels(scene: &GridScene, context: &cairo::Context, day_width: f64, foreground: Rgb) {
    set_source(context, foreground);
    select_font(context, 10.0, cairo::FontWeight::Normal);
    context.move_to(8.0, 28.0);
    let _ = context.show_text(&format!(
        "{} {}",
        l10n::calendar_week_short(),
        scene.week_number
    ));
    select_font(context, 12.0, cairo::FontWeight::Bold);
    for (index, day) in scene.days.iter().enumerate() {
        set_source(
            context,
            if day.is_today {
                Rgb::from_hex("#16598D")
            } else {
                foreground
            },
        );
        context.move_to(GUTTER + pixels(index) * day_width + 8.0, 28.0);
        let _ = context.show_text(&day.label);
    }
    set_source(context, foreground);
    select_font(context, 10.0, cairo::FontWeight::Normal);
    for hour in 1..24 {
        context.move_to(
            8.0,
            scene.content_top() + f64::from(hour) * scene.hour_height + 4.0,
        );
        let _ = context.show_text(&clock(hour * 60, scene.use_24_hour));
    }
}

fn draw_bands(scene: &GridScene, context: &cairo::Context, day_width: f64, foreground: Rgb) {
    for band in &scene.bands {
        let rect = Rect {
            x: GUTTER + pixels(band.day) * day_width + 1.0,
            y: HEADING_HEIGHT + f64::from(band.lane) * LANE_HEIGHT + 1.0,
            width: pixels(band.days) * day_width - 2.0,
            height: LANE_HEIGHT - 2.0,
        };
        paint::fill_rect(context, rect, band.background, band.awaiting);
        paint::hatch_and_dash(context, rect, band.background, band.awaiting);
        set_source(context, band.foreground);
        clipped_text(context, rect, &band.title, 11.0);
    }
    for (day, hidden) in scene.hidden_per_day.iter().copied().enumerate() {
        if hidden == 0 {
            continue;
        }
        set_source(context, foreground);
        let y =
            HEADING_HEIGHT + f64::from(scene.banner_lanes.saturating_sub(1)) * LANE_HEIGHT + 17.0;
        context.move_to(GUTTER + pixels(day) * day_width + 6.0, y);
        let _ = context.show_text(&l10n::calendar_all_day_more(i64::from(hidden)));
    }
}

fn draw_events(scene: &GridScene, context: &cairo::Context, day_width: f64) {
    for event in &scene.events {
        let lane_width = day_width / f64::from(event.columns);
        let rect = Rect {
            x: GUTTER + pixels(event.day) * day_width + f64::from(event.column) * lane_width + 1.0,
            y: scene.content_top() + f64::from(event.start_minutes) * scene.hour_height / 60.0,
            width: lane_width - 2.0,
            height: f64::from(event.end_minutes - event.start_minutes) * scene.hour_height / 60.0,
        };
        let visible_bottom = scene.viewport_top + scene.viewport_height;
        if rect.y + rect.height < scene.viewport_top || rect.y > visible_bottom {
            continue;
        }
        paint::fill_rect(context, rect, event.background, event.awaiting);
        paint::hatch(context, rect, event.border, event.awaiting);
        set_source(context, event.border);
        paint::set_dash(context, event.awaiting);
        context.rectangle(rect.x, rect.y, rect.width, rect.height.max(2.0));
        let _ = context.stroke();
        paint::set_dash(context, false);
        if rect.height >= 14.0 {
            set_source(context, event.foreground);
            clipped_text(
                context,
                rect,
                &event.title,
                if rect.height < 28.0 { 9.0 } else { 11.0 },
            );
        }
    }
}

fn draw_now(scene: &GridScene, context: &cairo::Context, width: f64, day_width: f64) {
    let Some(index) = scene.days.iter().position(|day| day.is_today) else {
        return;
    };
    let Some((_, minutes)) = now_in(&scene.timezone) else {
        return;
    };
    let minutes = f64::from(minutes);
    let y = scene.content_top() + minutes * scene.hour_height / 60.0;
    set_source(context, Rgb::from_hex("#c01c28"));
    context.set_line_width(2.0);
    context.move_to(GUTTER + pixels(index) * day_width, y);
    context.line_to((GUTTER + pixels(index + 1) * day_width).min(width), y);
    let _ = context.stroke();
}

fn draw_create(scene: &GridScene, context: &cairo::Context, width: f64, day_width: f64) {
    let Some(drag) = scene.drag() else {
        return;
    };
    let Some(paint) = scene.create else {
        return;
    };
    let live = drag.live_preview();
    let settled = drag.preview();
    let body = Rect {
        x: GUTTER + pixels(live.day) * day_width + 1.0,
        y: scene.content_top() + f64::from(live.start_minutes) * scene.hour_height / 60.0 + 1.0,
        width: (day_width - 2.0).max(1.0),
        height: (f64::from(live.minutes()) * scene.hour_height / 60.0 - 2.0).max(1.0),
    };
    paint::fill_rect(context, body, paint.background, false);
    set_source(context, paint.border);
    context.set_line_width(2.0);
    context.rectangle(body.x, body.y, body.width, body.height);
    let _ = context.stroke();
    draw_create_readout(
        scene,
        context,
        width,
        body,
        &format!(
            "{}–{}",
            clock(settled.start_minutes, scene.use_24_hour),
            clock(settled.end_minutes, scene.use_24_hour)
        ),
    );
}

fn draw_create_readout(
    scene: &GridScene,
    context: &cairo::Context,
    width: f64,
    body: Rect,
    text: &str,
) {
    select_font(context, 12.0, cairo::FontWeight::Normal);
    let Ok(extents) = context.text_extents(text) else {
        return;
    };
    let pill_width = extents.width() + 16.0;
    let pill_height = extents.height() + 8.0;
    let visible_top = scene.viewport_top;
    let visible_bottom = (visible_top + scene.viewport_height).min(scene.height());
    if width - GUTTER < pill_width || visible_bottom - visible_top < pill_height {
        return;
    }
    let x =
        (body.x + (body.width - pill_width) / 2.0).clamp(GUTTER, (width - pill_width).max(GUTTER));
    let above = body.y - pill_height - 6.0;
    let preferred = if above >= visible_top {
        above
    } else {
        body.y + body.height + 6.0
    };
    let y = preferred.clamp(visible_top, (visible_bottom - pill_height).max(visible_top));
    rounded_rect(context, x, y, pill_width, pill_height, pill_height / 2.0);
    set_source(
        context,
        if scene.dark {
            Rgb::from_hex("#e3e3e3")
        } else {
            Rgb::from_hex("#303030")
        },
    );
    let _ = context.fill();
    set_source(
        context,
        if scene.dark {
            Rgb::from_hex("#1b1b1b")
        } else {
            Rgb::from_hex("#f2f2f2")
        },
    );
    context.move_to(x + 8.0 - extents.x_bearing(), y + 4.0 - extents.y_bearing());
    let _ = context.show_text(text);
}

fn rounded_rect(context: &cairo::Context, x: f64, y: f64, width: f64, height: f64, radius: f64) {
    use std::f64::consts::{FRAC_PI_2, PI};

    context.new_sub_path();
    context.arc(x + width - radius, y + radius, radius, -FRAC_PI_2, 0.0);
    context.arc(
        x + width - radius,
        y + height - radius,
        radius,
        0.0,
        FRAC_PI_2,
    );
    context.arc(x + radius, y + height - radius, radius, FRAC_PI_2, PI);
    context.arc(x + radius, y + radius, radius, PI, PI + FRAC_PI_2);
    context.close_path();
}

fn clipped_text(context: &cairo::Context, rect: Rect, text: &str, size: f64) {
    let _ = context.save();
    context.rectangle(
        rect.x + 3.0,
        rect.y + 2.0,
        (rect.width - 6.0).max(0.0),
        rect.height - 4.0,
    );
    context.clip();
    select_font(context, size, cairo::FontWeight::Bold);
    context.move_to(rect.x + 4.0, rect.y + size + 3.0);
    let _ = context.show_text(text);
    let _ = context.restore();
}

fn theme_color(dark: bool, foreground: bool) -> Rgb {
    match (dark, foreground) {
        (true, true) => Rgb::new(0.95, 0.95, 0.95),
        (true, false) | (false, true) => Rgb::new(0.12, 0.12, 0.12),
        (false, false) => Rgb::new(1.0, 1.0, 1.0),
    }
}

fn select_font(context: &cairo::Context, size: f64, weight: cairo::FontWeight) {
    context.select_font_face("Nunito", cairo::FontSlant::Normal, weight);
    context.set_font_size(size);
}
