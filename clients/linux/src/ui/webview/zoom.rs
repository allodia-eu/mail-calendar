//! Pinch and Ctrl+scroll zoom for the reading view (`docs/reading-zoom.md`).

use std::{cell::Cell, rc::Rc};

use gtk::{glib, prelude::*};
use webkit6::{WebView, prelude::*};

/// The range the reader may zoom the message to, matching what a browser offers.
const ZOOM_RANGE: std::ops::RangeInclusive<f64> = 0.25..=5.0;

/// How far a held pinch must have moved before the message is laid out again.
///
/// `zoom-level` is a **page** zoom: each change re-lays-out the whole document in the web process,
/// which is a great deal more work than a touchscreen's report rate. Spent on every
/// `scale-changed`, the queued layouts outrun what the engine can do and the message crawls along
/// behind the fingers. So a held pinch buys a layout only where it has moved enough to see, and the
/// scale the gesture actually ended on is applied in full when it ends.
const ZOOM_GESTURE_STEP: f64 = 0.05;

/// Names on our own zoom controllers.
///
/// Not decoration: **WebKitGTK installs a `GtkGestureZoom` of its own on every `WebView`**, so
/// `observe_controllers()` answers with one whether or not we added anything. A test that looks for
/// "a `GestureZoom`" therefore finds the toolkit's and passes over a host that installs none, which
/// is how the composer briefly appeared to have a zoom it never had. Ours are identified by name.
pub(super) const PINCH_CONTROLLER: &str = "mailcal-reading-pinch";
pub(super) const SCROLL_CONTROLLER: &str = "mailcal-reading-ctrl-scroll";

/// Pinch and Ctrl+scroll zoom for the reading view (`docs/reading-zoom.md`). WebKitGTK carries a
/// `zoom-level` but binds nothing of ours to it, so unlike the other three hosts this one has to
/// supply both gestures itself.
///
/// ⚠️ **Both controllers must be in the `Capture` phase.** The web view claims a pinch and a scroll
/// for its own handling, so a controller left in the default `Bubble` phase is never reached and
/// the gesture silently does nothing. A scroll without Ctrl still `Proceed`s, so ordinary scrolling
/// reaches the page untouched.
pub(super) fn install_zoom_gestures(view: &WebView) {
    let zoom = gtk::GestureZoom::new();
    zoom.set_name(Some(PINCH_CONTROLLER));
    zoom.set_propagation_phase(gtk::PropagationPhase::Capture);
    // The scale a pinch reports is relative to where that pinch began, not to 1, so the level it
    // started from has to be captured when it does.
    let started_at = Rc::new(Cell::new(1.0_f64));
    // The level the gesture has asked for, whether or not it has been paid for yet.
    let asked_for = Rc::new(Cell::new(1.0_f64));
    // Weak, in every handler here: the view holds these controllers, so a strong reference would
    // keep it, and its web process, alive after its window has gone.
    let begin_view = view.downgrade();
    let begin_level = Rc::clone(&started_at);
    let begin_asked = Rc::clone(&asked_for);
    zoom.connect_begin(move |gesture, _| {
        // The claim is what keeps the pinch ours, on a touchscreen and on a touchpad alike.
        // WebKitGTK's own `GtkGestureZoom` drives its magnification: the rendered page is scaled
        // on the compositor while the gesture is held and committed to a page scale when it ends.
        // An unclaimed gesture reports the event unhandled, it carries on past the capture phase
        // and both run, so the picture moves at the product of the two scales and the commit
        // leaves the page layer scaled with no offset for one frame, which the reader sees as the
        // message jumping to its top-left corner and back on release. `zoom-level` is the scale
        // `docs/reading-zoom.md` rules 4 and 5 are written against, and this makes it the only one.
        gesture.set_state(gtk::EventSequenceState::Claimed);
        let Some(begin_view) = begin_view.upgrade() else {
            return;
        };
        begin_level.set(begin_view.zoom_level());
        begin_asked.set(begin_view.zoom_level());
    });
    let pinch_view = view.downgrade();
    let pinch_asked = Rc::clone(&asked_for);
    let pinch_level = Rc::clone(&started_at);
    zoom.connect_scale_changed(move |_, scale| {
        let Some(pinch_view) = pinch_view.upgrade() else {
            return;
        };
        let level = clamp_zoom(pinch_level.get() * scale);
        pinch_asked.set(level);
        if (level - pinch_view.zoom_level()).abs() >= level * ZOOM_GESTURE_STEP {
            pinch_view.set_zoom_level(level);
        }
    });
    let end_view = view.downgrade();
    zoom.connect_end(move |_, _| {
        if let Some(end_view) = end_view.upgrade() {
            end_view.set_zoom_level(asked_for.get());
        }
    });
    view.add_controller(zoom);

    let scroll = gtk::EventControllerScroll::new(gtk::EventControllerScrollFlags::VERTICAL);
    scroll.set_name(Some(SCROLL_CONTROLLER));
    scroll.set_propagation_phase(gtk::PropagationPhase::Capture);
    let scroll_view = view.downgrade();
    scroll.connect_scroll(move |controller, _, dy| {
        if !controller
            .current_event_state()
            .contains(gtk::gdk::ModifierType::CONTROL_MASK)
        {
            return glib::Propagation::Proceed;
        }
        let Some(scroll_view) = scroll_view.upgrade() else {
            return glib::Propagation::Proceed;
        };
        // A tenth per notch, compounding, so a step covers the same proportion of the range
        // wherever the reader already is.
        let step = if dy < 0.0 { 1.1 } else { 1.0 / 1.1 };
        scroll_view.set_zoom_level(clamp_zoom(scroll_view.zoom_level() * step));
        glib::Propagation::Stop
    });
    view.add_controller(scroll);
}

/// Holds a zoom level inside `ZOOM_RANGE`. A pinch reports an unbounded scale, so without this a
/// long enough one leaves the message at a size with no gesture back.
pub(super) fn clamp_zoom(level: f64) -> f64 {
    level.clamp(*ZOOM_RANGE.start(), *ZOOM_RANGE.end())
}
