//! A page's event labels, laid out once and drawn from what was kept (`docs/calendar.md` §7).
//!
//! Shaping text is what a frame of the drawn grid costs, and none of it changes while the grid
//! scrolls; only the wrapping changes with a resize. So each label is laid out once per page and
//! type size, its fixed widths are kept with it, and a title's wrapping is kept per width on
//! [`LABEL_STEP`](super::super::blocks::LABEL_STEP)'s grid. What is kept is the render node, so a
//! frame replays glyphs it already shaped.

use std::collections::HashMap;

use gtk::{cairo, gdk, pango, prelude::*};

use super::{
    glyphs::Glyphs,
    scene::{BandPaint, EventPaint},
};
use crate::ui::calendar::{
    blocks::{self, BAR_WIDTH, GLYPH_GAP, LABEL_PADDING},
    paint::{Rect, Rgb},
};

/// The all-day bar's type size.
const BAND_FONT_SIZE: f64 = 11.0;

/// One run of text, shaped and recorded.
pub(super) struct Text {
    node: Option<gtk::gsk::RenderNode>,
    pub(super) width: f64,
    pub(super) height: f64,
    /// Where each line ends, from the top: the height that keeps exactly that many whole lines.
    pub(super) line_bottoms: Vec<f64>,
}

impl Text {
    fn new(layout: &pango::Layout, color: &gdk::RGBA) -> Self {
        let (_, logical) = layout.pixel_extents();
        let snapshot = gtk::Snapshot::new();
        snapshot.append_layout(layout, color);
        let mut line_bottoms = Vec::new();
        let mut lines = layout.iter();
        loop {
            let (_, line) = lines.line_extents();
            line_bottoms.push(f64::from(line.y() + line.height()) / f64::from(pango::SCALE));
            if !lines.next_line() {
                break;
            }
        }
        Self {
            node: snapshot.to_node(),
            width: f64::from(logical.width()),
            height: f64::from(logical.height()),
            line_bottoms,
        }
    }

    /// The height of the first `lines` lines, or of all of them where there are fewer.
    pub(super) fn height_of(&self, lines: u32) -> f64 {
        let index = usize::try_from(lines).unwrap_or(usize::MAX).max(1) - 1;
        self.line_bottoms
            .get(index)
            .or_else(|| self.line_bottoms.last())
            .copied()
            .unwrap_or(self.height)
    }

    fn draw(&self, context: &cairo::Context, x: f64, y: f64) {
        let Some(node) = &self.node else {
            return;
        };
        let _ = context.save();
        context.translate(x, y);
        node.draw(context);
        let _ = context.restore();
    }
}

/// One event's label at one type size.
pub(super) struct Label {
    layout: pango::Layout,
    color: gdk::RGBA,
    /// The title on one line, unbounded: what a block too narrow for its widest word draws.
    single: Text,
    /// The height of one line of the title, which is where a whole line ends.
    pub(super) line: f64,
    /// The widest word of the title: it wraps only where every word fits its line.
    pub(super) widest: f64,
    time: Option<Text>,
    range: Option<Text>,
    wrapped: HashMap<i32, Text>,
}

impl Label {
    pub(super) fn new(context: &pango::Context, title: &str, size: f64, color: Rgb) -> Self {
        let layout = pango::Layout::new(context);
        layout.set_font_description(Some(&font(context, size, pango::Weight::Semibold)));
        #[allow(clippy::cast_possible_truncation)]
        let color = gdk::RGBA::new(color.red as f32, color.green as f32, color.blue as f32, 1.0);
        let widest = title
            .split_whitespace()
            .map(|word| {
                layout.set_text(word);
                f64::from(layout.pixel_extents().1.width())
            })
            .fold(0.0, f64::max);
        layout.set_text(title);
        let single = Text::new(&layout, &color);
        Self {
            line: single.height,
            single,
            widest,
            layout,
            color,
            time: None,
            range: None,
            wrapped: HashMap::new(),
        }
    }

    fn with_time(mut self, context: &pango::Context, event: &EventPaint, size: f64) -> Self {
        let layout = pango::Layout::new(context);
        layout.set_font_description(Some(&font(context, size, pango::Weight::Normal)));
        layout.set_text(&event.time);
        self.time = Some(Text::new(&layout, &self.color));
        layout.set_text(&event.range);
        self.range = Some(Text::new(&layout, &self.color));
        self
    }

    /// The title wrapped by word to `width`, a multiple of the label step.
    pub(super) fn wrapped(&mut self, width: f64) -> &Text {
        let (layout, color) = (&self.layout, &self.color);
        self.wrapped
            .entry(blocks::wrap_key(width))
            .or_insert_with(|| {
                #[allow(clippy::cast_possible_truncation)]
                layout.set_width((width * f64::from(pango::SCALE)) as i32);
                layout.set_wrap(pango::WrapMode::Word);
                let text = Text::new(layout, color);
                layout.set_width(-1);
                text
            })
    }
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
enum Key {
    /// A timed event, by its place in the page, at a type size in tenths of a unit.
    Event(usize, u32),
    Band(usize),
}

/// The labels of one page, cleared when the page changes.
#[derive(Default)]
pub(super) struct Labels {
    held: HashMap<Key, Label>,
}

impl Labels {
    pub(super) fn clear(&mut self) {
        self.held.clear();
    }

    /// How many labels have been laid out since the page last changed.
    #[cfg(test)]
    pub(super) fn len(&self) -> usize {
        self.held.len()
    }

    fn event(
        &mut self,
        context: &pango::Context,
        index: usize,
        event: &EventPaint,
        size: f64,
    ) -> &mut Label {
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let key = Key::Event(index, (size * 10.0) as u32);
        self.held.entry(key).or_insert_with(|| {
            Label::new(context, &event.title, size, event.colors.text)
                .with_time(context, event, size)
        })
    }

    fn band(&mut self, context: &pango::Context, index: usize, band: &BandPaint) -> &mut Label {
        self.held
            .entry(Key::Band(index))
            .or_insert_with(|| Label::new(context, &band.title, BAND_FONT_SIZE, band.colors.text))
    }
}

/// What one paint draws its labels with: the surface's own text context, the page's labels and
/// its glyphs.
pub(super) struct Ink<'a> {
    pub(super) pango: pango::Context,
    pub(super) labels: &'a mut Labels,
    pub(super) glyphs: Glyphs,
}

impl<'a> Ink<'a> {
    pub(super) fn of(widget: &impl IsA<gtk::Widget>, labels: &'a mut Labels) -> Self {
        Self {
            pango: widget.pango_context(),
            labels,
            glyphs: Glyphs::of(widget),
        }
    }
}

/// A timed block's label: the title wrapped over every whole line the block has room for, the
/// time beneath it, and the meeting and repeat glyphs in the top-right corner. A block too short
/// for one line draws none: a title cut through the middle reads as a broken grid (§4).
pub(super) fn draw_event(
    context: &cairo::Context,
    ink: &mut Ink<'_>,
    index: usize,
    event: &EventPaint,
    rect: Rect,
    hour_height: f64,
) {
    let minutes = event.end_minutes.saturating_sub(event.start_minutes);
    let lines = blocks::title_lines(minutes, hour_height);
    if lines == 0 {
        return;
    }
    let size = blocks::font_size(minutes);
    let top = rect.y + blocks::inset(minutes) - blocks::GAP;
    let left = rect.x + BAR_WIDTH + LABEL_PADDING;
    let width = (rect.x + rect.width - LABEL_PADDING - left).max(0.0);
    let glyphs = &ink.glyphs;
    let label = ink.labels.event(&ink.pango, index, event, size);
    let _ = context.save();
    context.rectangle(rect.x, rect.y, rect.width, rect.height);
    context.clip();

    // In the top-right corner, where Apple's Calendar keeps its own, so the title wraps beside
    // them; and only while the title keeps room of its own.
    let count = u32::from(event.has_conference) + u32::from(event.repeats);
    let glyphs_width = blocks::glyphs_width(count, size);
    let available = blocks::beside_glyphs(width, glyphs_width).map_or(width, |room| {
        let mut x = left + width - glyphs_width;
        let y = top + ((label.line - size) / 2.0).max(0.0);
        let color = event.colors.text;
        if event.has_conference {
            glyphs.camera.draw(context, x, y, size, color);
            x += size + GLYPH_GAP;
        }
        if event.repeats {
            glyphs.repeat.draw(context, x, y, size, color);
        }
        room
    });

    let wrap = blocks::wrap_width(available);
    let line = label.line;
    let title_height = if blocks::wraps(label.widest, wrap) {
        let text = label.wrapped(wrap);
        // Cut after the last whole line there is room for, where Pango ended that line, so
        // nothing of the next line shows beneath it.
        let height = text.height_of(lines);
        clipped(
            context,
            Rect {
                x: left,
                y: top,
                width: available,
                height,
            },
            |context| {
                text.draw(context, left, top);
            },
        );
        height
    } else {
        // A word wider than the block would be broken across two lines, so the title runs on as
        // one line and is cut at the block's edge instead.
        let text = &label.single;
        clipped(
            context,
            Rect {
                x: left,
                y: top,
                width: available,
                height: line,
            },
            |context| {
                text.draw(context, left, top);
            },
        );
        line
    };

    if blocks::shows_time(minutes, hour_height)
        && let (Some(time), Some(range)) = (&label.time, &label.range)
    {
        let shown = if blocks::shows_range(range.width, width) {
            range
        } else {
            time
        };
        let height = blocks::line_height(minutes);
        let y = top + title_height;
        clipped(
            context,
            Rect {
                x: left,
                y,
                width,
                height,
            },
            |context| {
                shown.draw(context, left, y);
            },
        );
    }
    let _ = context.restore();
}

/// An all-day bar's title on one line, and the camera after it for an event with a video meeting.
/// The title gives up the camera's room first, so the camera stays in sight; a bar too narrow for
/// both draws the title alone, and its spoken label still says "video meeting".
pub(super) fn draw_band(
    context: &cairo::Context,
    ink: &mut Ink<'_>,
    index: usize,
    band: &BandPaint,
    rect: Rect,
) {
    let glyphs = &ink.glyphs;
    let label = ink.labels.band(&ink.pango, index, band);
    let left = rect.x + BAR_WIDTH + LABEL_PADDING;
    let room = (rect.x + rect.width - LABEL_PADDING - left).max(0.0);
    let size = BAND_FONT_SIZE;
    let reserve = size + GLYPH_GAP;
    let camera = band.has_conference && room - reserve >= blocks::MINIMUM_TITLE_WIDTH;
    let title_room = if camera { room - reserve } else { room };
    let top = rect.y + ((rect.height - label.line) / 2.0).max(0.0);
    let text = &label.single;
    clipped(
        context,
        Rect {
            x: left,
            y: rect.y,
            width: title_room,
            height: rect.height,
        },
        |context| text.draw(context, left, top),
    );
    if camera {
        let x = left + text.width.min(title_room) + GLYPH_GAP;
        let y = top + ((label.line - size) / 2.0).max(0.0);
        glyphs.camera.draw(context, x, y, size, band.colors.text);
    }
}

fn clipped(context: &cairo::Context, rect: Rect, draw: impl FnOnce(&cairo::Context)) {
    let _ = context.save();
    context.rectangle(rect.x, rect.y, rect.width.max(0.0), rect.height.max(0.0));
    context.clip();
    draw(context);
    let _ = context.restore();
}

/// The surface's own typeface at `size` units and `weight`.
fn font(context: &pango::Context, size: f64, weight: pango::Weight) -> pango::FontDescription {
    let mut font = context.font_description().unwrap_or_default();
    font.set_weight(weight);
    font.set_absolute_size(size * f64::from(pango::SCALE));
    font
}
