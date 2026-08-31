//! The meeting-day preview: the user's own day, so a clash can be *seen* rather than described.
//!
//! Laid out by the same solver as every other calendar page (`docs/calendar.md` §1), so this only
//! multiplies. What it decides is the **band**: the meeting, everything that overlaps it, and an
//! hour of air; never the whole day, which squeezes an hour under ten points and draws the
//! meeting the card is about as an *unnamed* rectangle beside a named one.
//!
//! Drawn rather than composed, like the time grid, with a `GtkFixed` of transparent labels over it
//! so every block still has a spoken label; a Cairo surface is one node to a screen reader
//! otherwise (`docs/calendar.md` §4). Nothing here is tappable: this is a picture of a day, not a
//! calendar.

use std::{cell::RefCell, rc::Rc};

use adw::prelude::*;
use gtk::{accessible::Property as AccessibleProperty, cairo};
use mailcal_bindings::InvitationPreview;

use super::{
    HourSpan, MINIMUM_TITLED_HEIGHT, MinuteSpan, meeting_minute_span, preview_height, preview_span,
    preview_stride,
};
use crate::{
    l10n,
    ui::calendar::{
        date::clock,
        paint::{self, Rect, Rgb},
    },
};

/// The hour-label column. Narrower than the full grid's: this box sits inside a card.
const GUTTER: f64 = 44.0;

/// One stacked row of the all-day banner above the hours.
const LANE_HEIGHT: f64 = 18.0;

/// One block of the preview, in the band's own coordinates.
#[derive(Clone, Debug)]
struct Block {
    title: String,
    spoken: String,
    start_minutes: u32,
    end_minutes: u32,
    column: u32,
    columns: u32,
    awaiting: bool,
}

/// One all-day bar above the hours.
#[derive(Clone, Debug)]
struct Band {
    title: String,
    spoken: String,
    lane: u32,
    awaiting: bool,
}

/// Everything a frame needs, derived once from the core's preview.
#[derive(Clone, Debug)]
struct Scene {
    hours: HourSpan,
    hour_height: f64,
    blocks: Vec<Block>,
    bands: Vec<Band>,
    banner_lanes: u32,
    use_24_hour: bool,
    dark: bool,
}

impl Scene {
    fn empty() -> Self {
        Self {
            hours: HourSpan { first: 8, last: 14 },
            hour_height: 22.0,
            blocks: Vec::new(),
            bands: Vec::new(),
            banner_lanes: 0,
            use_24_hour: true,
            dark: false,
        }
    }

    fn build(
        preview: &InvitationPreview,
        meeting: MinuteSpan,
        use_24_hour: bool,
        dark: bool,
    ) -> Self {
        let others = preview
            .timed
            .iter()
            .map(|segment| MinuteSpan {
                start: segment.start_minutes,
                end: segment.end_minutes,
            })
            .collect::<Vec<_>>();
        let hours = preview_span(meeting, &others);
        let blocks = preview
            .timed
            .iter()
            .map(|segment| {
                let title = title(&segment.title);
                let range = format!(
                    "{}–{}",
                    clock(segment.start_minutes, use_24_hour),
                    clock(segment.end_minutes, use_24_hour)
                );
                Block {
                    spoken: paint::spoken_with_hold(
                        &format!("{title}, {range}"),
                        segment.participation,
                    ),
                    title,
                    start_minutes: segment.start_minutes,
                    end_minutes: segment.end_minutes,
                    column: segment.column,
                    columns: segment.columns.max(1),
                    awaiting: paint::is_awaiting(segment.participation),
                }
            })
            .collect();
        let bands = preview
            .all_day
            .iter()
            .map(|bar| {
                let title = title(&bar.title);
                Band {
                    spoken: paint::spoken_with_hold(
                        &format!("{title}, {}", l10n::calendar_all_day()),
                        bar.participation,
                    ),
                    title,
                    lane: bar.lane,
                    awaiting: paint::is_awaiting(bar.participation),
                }
            })
            .collect();
        let hours_count = hours.count().max(1);
        Self {
            hours,
            hour_height: preview_height(hours_count) / f64::from(hours_count),
            blocks,
            bands,
            banner_lanes: preview.all_day_lanes,
            use_24_hour,
            dark,
        }
    }

    /// Where the hours start, under the all-day banner.
    fn content_top(&self) -> f64 {
        f64::from(self.banner_lanes) * LANE_HEIGHT
    }

    fn height(&self) -> f64 {
        self.content_top() + f64::from(self.hours.count().max(1)) * self.hour_height
    }

    /// A wall-clock minute's y within the band.
    ///
    /// Nothing the card counts can fall outside it: a conflict is by definition an event
    /// overlapping the meeting's window, so every one of them widened the band already.
    fn y_of(&self, minutes: u32) -> f64 {
        let first = f64::from(self.hours.first) * 60.0;
        self.content_top() + (f64::from(minutes) - first) * self.hour_height / 60.0
    }
}

/// The drawn preview and its semantic overlay.
pub(super) struct PreviewGrid {
    root: gtk::Overlay,
    drawing: gtk::DrawingArea,
    labels: gtk::Fixed,
    scene: Rc<RefCell<Scene>>,
}

impl PreviewGrid {
    pub(super) fn new() -> Self {
        install_semantic_css();
        let drawing = gtk::DrawingArea::new();
        drawing.set_hexpand(true);
        let labels = gtk::Fixed::new();
        labels.set_hexpand(true);
        // A picture of a day: nothing in it is tappable, so the overlay carries labels only.
        labels.set_can_target(false);
        let root = gtk::Overlay::new();
        root.set_child(Some(&drawing));
        root.add_overlay(&labels);
        let scene = Rc::new(RefCell::new(Scene::empty()));
        let draw_scene = Rc::clone(&scene);
        drawing.set_draw_func(move |_, context, width, _| {
            draw(&draw_scene.borrow(), context, f64::from(width));
        });
        let resize_scene = Rc::clone(&scene);
        let resize_labels = labels.clone();
        drawing.connect_resize(move |_, width, _| {
            rebuild_labels(&resize_labels, &resize_scene.borrow(), f64::from(width));
        });
        Self {
            root,
            drawing,
            labels,
            scene,
        }
    }

    pub(super) fn widget(&self) -> &gtk::Overlay {
        &self.root
    }

    /// Redraws for `preview`, with the meeting the card is about placed in the same zone the core
    /// solved the day in; reading it in the display zone would put it in the wrong row of its own
    /// picture.
    pub(super) fn apply(
        &self,
        preview: &InvitationPreview,
        starts_at: &str,
        ends_at: &str,
        zone: &str,
        use_24_hour: bool,
    ) {
        let layout_zone = if preview.timezone.is_empty() {
            zone
        } else {
            preview.timezone.as_str()
        };
        let meeting = meeting_minute_span(starts_at, ends_at, layout_zone);
        let scene = Scene::build(
            preview,
            meeting,
            use_24_hour,
            adw::StyleManager::default().is_dark(),
        );
        let height = scene.height();
        *self.scene.borrow_mut() = scene;
        self.drawing.set_content_height(pixel_size(height.ceil()));
        self.labels.set_size_request(-1, pixel_size(height.ceil()));
        let width = f64::from(self.drawing.width());
        if width > 0.0 {
            rebuild_labels(&self.labels, &self.scene.borrow(), width);
        }
        self.drawing.queue_draw();
    }
}

fn draw(scene: &Scene, context: &cairo::Context, width: f64) {
    let foreground = if scene.dark {
        Rgb::new(0.95, 0.95, 0.95)
    } else {
        Rgb::new(0.12, 0.12, 0.12)
    };
    let line = if scene.dark {
        Rgb::new(0.28, 0.28, 0.28)
    } else {
        Rgb::new(0.86, 0.86, 0.86)
    };
    let day_width = (width - GUTTER).max(1.0);
    let (fill, text, border) = paint::neutral_swatch();

    paint::set_source(context, line);
    context.set_line_width(1.0);
    let stride = preview_stride(scene.hour_height);
    for hour in scene.hours.first..=scene.hours.last {
        let y = scene.y_of(hour * 60);
        context.move_to(GUTTER, y);
        context.line_to(width, y);
    }
    let _ = context.stroke();
    paint::set_source(context, foreground);
    select_font(context, 10.0);
    for hour in scene.hours.first..scene.hours.last {
        if !(hour - scene.hours.first).is_multiple_of(stride) {
            continue;
        }
        context.move_to(6.0, scene.y_of(hour * 60) + 10.0);
        let _ = context.show_text(&clock(hour * 60, scene.use_24_hour));
    }

    for band in &scene.bands {
        let rect = Rect {
            x: GUTTER + 1.0,
            y: f64::from(band.lane) * LANE_HEIGHT + 1.0,
            width: day_width - 2.0,
            height: LANE_HEIGHT - 2.0,
        };
        paint::fill_rect(context, rect, fill, band.awaiting);
        paint::hatch_and_dash(context, rect, fill, band.awaiting);
        paint::set_source(context, text);
        clipped_text(context, rect, &band.title, 9.0);
    }

    for block in &scene.blocks {
        let lane_width = day_width / f64::from(block.columns);
        let top = scene.y_of(block.start_minutes);
        let bottom = scene.y_of(block.end_minutes);
        let rect = Rect {
            x: GUTTER + f64::from(block.column) * lane_width + 1.0,
            y: top,
            width: lane_width - 2.0,
            height: (bottom - top).max(2.0),
        };
        paint::fill_rect(context, rect, fill, block.awaiting);
        paint::hatch(context, rect, border, block.awaiting);
        paint::set_source(context, border);
        paint::set_dash(context, block.awaiting);
        context.rectangle(rect.x, rect.y, rect.width, rect.height);
        let _ = context.stroke();
        paint::set_dash(context, false);
        // Below the threshold a block goes untitled rather than carrying a title sliced through
        // the middle. Never clipped; and the spoken label is there either way.
        if rect.height >= MINIMUM_TITLED_HEIGHT {
            paint::set_source(context, text);
            clipped_text(context, rect, &block.title, 9.0);
        }
    }
}

/// The transparent AT-SPI nodes over the drawing; one per block, none of them a target.
fn rebuild_labels(fixed: &gtk::Fixed, scene: &Scene, width: f64) {
    while let Some(child) = fixed.first_child() {
        fixed.remove(&child);
    }
    let day_width = (width - GUTTER).max(1.0);
    for band in &scene.bands {
        put_label(
            fixed,
            &band.spoken,
            GUTTER + 1.0,
            f64::from(band.lane) * LANE_HEIGHT + 1.0,
            day_width - 2.0,
            LANE_HEIGHT - 2.0,
        );
    }
    for block in &scene.blocks {
        let lane_width = day_width / f64::from(block.columns);
        let top = scene.y_of(block.start_minutes);
        let bottom = scene.y_of(block.end_minutes);
        put_label(
            fixed,
            &block.spoken,
            GUTTER + f64::from(block.column) * lane_width + 1.0,
            top,
            lane_width - 2.0,
            (bottom - top).max(2.0),
        );
    }
}

fn put_label(fixed: &gtk::Fixed, spoken: &str, x: f64, y: f64, width: f64, height: f64) {
    let label = gtk::Label::new(Some(spoken));
    label.add_css_class("invitation-preview-label");
    label.update_property(&[AccessibleProperty::Label(spoken)]);
    label.set_size_request(pixel_size(width.max(1.0)), pixel_size(height.max(1.0)));
    fixed.put(&label, x, y);
}

fn title(value: &str) -> String {
    if value.trim().is_empty() {
        l10n::event_no_title().to_owned()
    } else {
        value.to_owned()
    }
}

fn clipped_text(context: &cairo::Context, rect: Rect, text: &str, size: f64) {
    let _ = context.save();
    context.rectangle(
        rect.x + 5.0,
        rect.y + 1.0,
        (rect.width - 8.0).max(0.0),
        (rect.height - 2.0).max(0.0),
    );
    context.clip();
    select_font(context, size);
    context.move_to(rect.x + 6.0, rect.y + size + 1.0);
    let _ = context.show_text(text);
    let _ = context.restore();
}

fn select_font(context: &cairo::Context, size: f64) {
    context.select_font_face("Nunito", cairo::FontSlant::Normal, cairo::FontWeight::Bold);
    context.set_font_size(size);
}

/// Preview geometry is a few hundred pixels; clamp before the GTK integer boundary.
#[expect(
    clippy::cast_possible_truncation,
    reason = "clamped into i32's range on the line above the cast"
)]
fn pixel_size(value: f64) -> i32 {
    value.clamp(1.0, f64::from(i32::MAX)).round() as i32
}

fn install_semantic_css() {
    static INSTALLED: std::sync::Once = std::sync::Once::new();
    INSTALLED.call_once(|| {
        let provider = gtk::CssProvider::new();
        provider.load_from_string(".invitation-preview-label { color: transparent; }");
        if let Some(display) = gtk::gdk::Display::default() {
            gtk::style_context_add_provider_for_display(
                &display,
                &provider,
                gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
            );
        }
    });
}
