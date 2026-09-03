//! The page a message is drawn on, behind the reading pane's body area.
//!
//! The colour is the core's ([`mailcal_bindings::message_canvas`]), the same one
//! [`mailcal_bindings::render_message_html`] gives the document, so the sheet the pane paints and
//! the page the WebView paints inside it are one white rather than two.
//!
//! The pane draws it for the whole of an open, not only once a body has arrived. Leaving the
//! waiting gap transparent punches a hole in the page: the body area went white, black, white on
//! every message opened against a dark theme, for as long as the open took (75–82 ms, measured),
//! which reads as a flicker rather than as a message opening. `../../../../docs/sync-progress.md`
//! binds every client to this.

use std::sync::Once;

use adw::prelude::*;

/// The style class carrying the canvas. On the body stack, so every page of it (the gap before the
/// body lands, a plain-text body, the spinner, a load error) is the same sheet.
pub(super) const CANVAS_CLASS: &str = "mailcal-message-canvas";

/// Installs the canvas stylesheet once per display.
pub(super) fn install_styles() {
    static INSTALLED: Once = Once::new();
    if let Some(display) = gtk::gdk::Display::default() {
        INSTALLED.call_once(|| {
            let provider = gtk::CssProvider::new();
            provider.load_from_string(&canvas_css());
            gtk::style_context_add_provider_for_display(
                &display,
                &provider,
                gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
            );
        });
    }
}

/// `color` as well as `background`: GTK inherits it, so the spinner, the plain-text body and the
/// load error's wording are all legible ink on the sheet rather than the dark theme's own text
/// colour on white.
fn canvas_css() -> String {
    let canvas = mailcal_bindings::message_canvas();
    format!(
        ".{CANVAS_CLASS} {{ background: {}; color: {}; }}",
        canvas.background, canvas.foreground
    )
}

/// The canvas as a colour, for anything that needs one rather than a stylesheet.
pub(crate) fn canvas_rgba() -> gtk::gdk::RGBA {
    let canvas = mailcal_bindings::message_canvas();
    canvas
        .background
        .parse()
        .expect("the core's canvas is `#rrggbb`, which GDK parses")
}

/// Whether the body area is showing a message's page at all: false only with nothing open, where
/// there is no message and so no page to draw.
pub(super) fn set_drawn(stack: &gtk::Stack, drawn: bool) {
    if drawn {
        stack.add_css_class(CANVAS_CLASS);
    } else {
        stack.remove_css_class(CANVAS_CLASS);
    }
}

/// The colour GTK would actually paint at `(x, y)` of `widget`, as `#rrggbb`.
///
/// A style class is not an oracle: it reads back whatever was set on the widget whether or not a
/// rule matched it, so asserting on one is a green light for a body area that is still a hole.
/// This renders the widget the way the frame does and reads the pixel out of the texture.
#[cfg(test)]
pub(super) fn painted_hex(widget: &impl IsA<gtk::Widget>, x: usize, y: usize) -> String {
    const SIZE: usize = 64;
    let paintable = gtk::WidgetPaintable::new(Some(widget.as_ref()));
    let snapshot = gtk::Snapshot::new();
    #[allow(clippy::cast_precision_loss)]
    paintable.snapshot(&snapshot, SIZE as f64, SIZE as f64);
    let renderer = gtk::gsk::CairoRenderer::new();
    renderer
        .realize(gtk::gdk::Surface::NONE)
        .expect("a Cairo renderer needs no surface");
    // Sized from the texture rather than from `SIZE`: `render_texture` renders the node's own
    // bounds, which a widget drawing outside its allocation makes larger than what was asked
    // for, and `download` writes `stride * height` bytes whatever the buffer holds.
    let painted = snapshot.to_node().map(|node| {
        let texture = renderer.render_texture(&node, None);
        let stride = usize::try_from(texture.width()).expect("a texture width fits a usize") * 4;
        let height = usize::try_from(texture.height()).expect("a texture height fits a usize");
        let mut pixels = vec![0u8; stride * height];
        texture.download(&mut pixels, stride);
        (pixels, stride)
    });
    // A realised renderer aborts the process on drop, so this runs before any early return.
    renderer.unrealize();
    // Nothing painted, or a pixel no one painted: the widget drew no page of its own.
    let Some((pixels, stride)) = painted else {
        return String::new();
    };
    let at = y * stride + x * 4;
    if at + 3 >= pixels.len() {
        return String::new();
    }
    if pixels[at + 3] == 0 {
        return String::new();
    }
    // `download` hands back BGRA.
    format!(
        "#{:02x}{:02x}{:02x}",
        pixels[at + 2],
        pixels[at + 1],
        pixels[at]
    )
}

#[cfg(test)]
pub(crate) mod tests {
    use adw::prelude::*;

    use super::{CANVAS_CLASS, canvas_css, painted_hex, set_drawn};

    /// The class is not decoration: drawn, the body area paints the core's page; not drawn, it
    /// paints nothing and whatever is behind the pane shows through.
    ///
    /// This is what lets the pane's own test assert on the class. Nothing else here can see a
    /// stylesheet that stopped matching: the class reads back from the widget whether or not a
    /// rule ever applied to it, so on its own it is a green light for a body area still full of
    /// holes.
    pub(crate) fn the_drawn_canvas_paints_the_page_the_core_names() {
        let canvas = mailcal_bindings::message_canvas();
        assert_eq!(body_area(true), canvas.background);
        assert_eq!(
            body_area(false),
            "",
            "with nothing open the pane draws no page, so the chrome behind it shows"
        );
    }

    /// What a body area with the canvas `drawn`, or not, actually paints.
    ///
    /// A fresh window each time, with the class set before it is presented: a class added to a
    /// live widget changes what it paints only once its style is revalidated, and an offscreen
    /// test has no frame clock to do that on demand, so toggling one in place reads back the
    /// frame it replaced, and both halves of this would pass while painting the same thing.
    fn body_area(drawn: bool) -> String {
        let stack = gtk::Stack::new();
        stack.add_named(&gtk::Box::new(gtk::Orientation::Vertical, 0), Some("body"));
        stack.set_visible_child_name("body");
        stack.set_size_request(40, 40);
        let window = adw::ApplicationWindow::builder().build();
        window.set_content(Some(&stack));
        super::install_styles();
        set_drawn(&stack, drawn);
        assert_eq!(stack.has_css_class(CANVAS_CLASS), drawn);
        window.present();
        while gtk::glib::MainContext::default().iteration(false) {}
        let painted = painted_hex(&stack, 2, 2);
        window.close();
        painted
    }

    /// The base the WebView presents until the document has painted is that same page, opaque.
    ///
    /// Decoded here independently of `canvas_rgba`, so this compares the colour GDK was handed
    /// against the core's hex rather than against itself.
    pub(crate) fn the_web_view_base_is_the_same_page() {
        let canvas = mailcal_bindings::message_canvas();
        let hex = u32::from_str_radix(canvas.background.trim_start_matches('#'), 16)
            .expect("the canvas is `#rrggbb`");
        let channel =
            |shift: u32| f32::from(u8::try_from((hex >> shift) & 0xff).expect("one byte")) / 255.0;
        let rgba = super::canvas_rgba();
        assert!((rgba.red() - channel(16)).abs() < f32::EPSILON, "{rgba:?}");
        assert!((rgba.green() - channel(8)).abs() < f32::EPSILON, "{rgba:?}");
        assert!((rgba.blue() - channel(0)).abs() < f32::EPSILON, "{rgba:?}");
        assert!(
            (rgba.alpha() - 1.0).abs() < f32::EPSILON,
            "a page the document is about to paint over is opaque: {rgba:?}"
        );
    }

    #[test]
    fn the_sheet_is_the_core_s_canvas_rather_than_a_second_white() {
        let canvas = mailcal_bindings::message_canvas();
        let css = canvas_css();
        assert!(
            css.contains(&format!("background: {};", canvas.background)),
            "{css}"
        );
        assert!(
            css.contains(&format!("color: {};", canvas.foreground)),
            "{css}"
        );
        // The same page the document gets. A seam here would show on every message; before the
        // body lands it is the whole body area, so it would show as a flicker on every open.
        let document = mailcal_bindings::render_message_html("<p>x</p>".to_owned(), false);
        assert!(
            document.contains(&format!("background:{}", canvas.background)),
            "{document}"
        );
    }
}
