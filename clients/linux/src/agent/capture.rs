//! A picture of a window, drawn by the app itself.
//!
//! The widget renders its own snapshot through its window's renderer, so the picture is the frame
//! the app would present, at the display's scale, on any compositor. A compositor capture (`grim`)
//! sees what the compositor composed, popovers and all, which this cannot: a popover is a surface
//! of its own and is captured by passing its handle.

use std::path::PathBuf;

use gtk::{gdk, prelude::*};
use serde::Deserialize;
use serde_json::json;

use super::{
    protocol::{Answer, Failure, Target},
    tree,
};

/// How many frames a capture waits for one to be drawn: about a second.
const FRAMES: u32 = 60;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Screenshot {
    path: PathBuf,
    #[serde(default)]
    h: Option<u64>,
}

pub(super) async fn screenshot(args: Screenshot) -> Answer {
    let widget = match args.h {
        Some(h) => tree::widget(&Target { h })?,
        None => tree::toplevels()
            .into_iter()
            .next()
            .ok_or_else(|| Failure::new("no window is open", "no-element"))?,
    };
    if !widget.is_mapped() {
        return Err(Failure::new("the element is not on screen", "not-showing"));
    }
    // A window that has not drawn its first frame has nothing to give yet: wait for one.
    let mut texture = render(&widget);
    for _ in 0..FRAMES {
        if texture.is_ok() {
            break;
        }
        widget.queue_draw();
        gtk::glib::timeout_future(std::time::Duration::from_millis(16)).await;
        texture = render(&widget);
    }
    let texture = texture?;
    if let Some(parent) = args.path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| {
            Failure::new(format!("cannot create {}: {error}", parent.display()), "io")
        })?;
    }
    texture.save_to_png(&args.path).map_err(|error| {
        Failure::new(
            format!("cannot write {}: {error}", args.path.display()),
            "io",
        )
    })?;
    Ok(json!({
        "path": args.path,
        "width": texture.width(),
        "height": texture.height(),
    }))
}

/// The widget's frame as a texture in device pixels.
pub(super) fn render(widget: &gtk::Widget) -> Result<gdk::Texture, Failure> {
    let renderer = widget
        .native()
        .and_then(|native| native.renderer())
        .ok_or_else(|| Failure::new("the element has no renderer yet", "not-showing"))?;
    let (width, height) = (widget.width(), widget.height());
    if width <= 0 || height <= 0 {
        return Err(Failure::new("the element has no size", "not-showing"));
    }
    let scale = widget.scale_factor();
    let paintable = gtk::WidgetPaintable::new(Some(widget));
    let snapshot = gtk::Snapshot::new();
    #[allow(clippy::cast_precision_loss)] // A scale factor is a small integer.
    snapshot.scale(scale as f32, scale as f32);
    paintable.snapshot(
        snapshot.upcast_ref::<gdk::Snapshot>(),
        f64::from(width),
        f64::from(height),
    );
    let node = snapshot
        .to_node()
        .ok_or_else(|| Failure::new("the element drew nothing", "empty"))?;
    Ok(renderer.render_texture(node, None))
}
