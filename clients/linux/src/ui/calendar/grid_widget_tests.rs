use std::rc::Rc;

use adw::prelude::*;

use super::{
    GridScene, GridSurface, rebuild_hits_with,
    scene::{DayPaint, EventPaint},
    scroll::Framing,
    semantic_nodes_enabled,
};
use crate::ui::{
    AppInput,
    calendar::{EventIdentity, paint::Rgb},
};

pub(crate) fn the_create_drag_owns_the_primary_pointer_before_event_buttons() {
    let (sender, _receiver) = relm4::channel::<AppInput>();
    let surface = GridSurface::new(sender);
    let controllers = surface.hits.observe_controllers();
    let gesture = (0..controllers.n_items())
        .filter_map(|index| controllers.item(index))
        .find_map(|controller| controller.downcast::<gtk::GestureDrag>().ok())
        .expect("the calendar hit plane owns a drag controller");
    assert_eq!(gesture.propagation_phase(), gtk::PropagationPhase::Capture);
    assert_eq!(gesture.button(), gtk::gdk::BUTTON_PRIMARY);
}

pub(crate) fn recentring_releases_the_scene_before_value_notification() {
    let scene = Rc::new(std::cell::RefCell::new(GridScene::empty()));
    scene.borrow_mut().hour_height = 60.0;
    let adjustment = gtk::Adjustment::new(0.0, 0.0, 1492.0, 1.0, 60.0, 480.0);
    let notified_scene = Rc::clone(&scene);
    adjustment.connect_value_notify(move |adjustment| {
        notified_scene
            .borrow_mut()
            .set_viewport_top(adjustment.value());
    });
    let framing = Framing::default();
    framing.open();

    assert!(framing.seat_at(&adjustment, &scene, 12.0 * 60.0));

    assert!(!framing.is_pending());
    assert!((adjustment.value() - 532.0).abs() < f64::EPSILON);
    assert!((scene.borrow().viewport_top - 532.0).abs() < f64::EPSILON);
}

/// A scene tall enough to scroll, with one event so the overlay has a hit target to destroy.
fn scrollable_scene() -> GridScene {
    let mut scene = GridScene::empty();
    scene.days = vec![DayPaint {
        date: time::Date::from_calendar_date(2026, time::Month::August, 27).unwrap(),
        label: "Thu 27".to_owned(),
        is_today: true,
    }];
    scene.events = vec![EventPaint {
        identity: EventIdentity {
            account: "alice@test.local".to_owned(),
            key: "onboarding".to_owned(),
            occurrence: "2026-08-27T13:00:00".to_owned(),
        },
        title: "Onboarding".to_owned(),
        spoken: "Onboarding, 13:00-13:30, Work".to_owned(),
        day: 0,
        start_minutes: 780,
        end_minutes: 810,
        column: 0,
        columns: 1,
        background: Rgb::new(0.0, 0.0, 0.0),
        foreground: Rgb::new(1.0, 1.0, 1.0),
        border: Rgb::new(0.0, 0.0, 0.0),
        awaiting: false,
    }];
    scene.hidden_per_day = vec![0];
    scene.hour_height = 60.0;
    scene.visible_hours = 8;
    scene.is_materialized = true;
    scene
}

/// A click on an event must not park focus on a widget that is about to be destroyed.
///
/// Opening an event rebuilds this overlay, which destroys the hit button the click landed on.
/// GTK then moves focus off the dying widget and the scrolled window *animates* to reveal
/// whatever inherits it; the first target, at the top of the day; so the grid slid away from
/// the hours the reader was looking at. Measured on the running app: 884 → 53 over five frames.
///
/// The property is the defect, so the property is what is asserted. The scroll itself cannot be
/// the oracle here: it is driven by a focus change GTK only performs for a real toplevel focus,
/// and it arrives on an easing curve some frames after the rebuild returns; so a widget test
/// that watched the adjustment would sit at its start value and pass whatever the code did.
/// `can_focus` stays on beside it: a screen reader still has to reach every event.
pub(crate) fn a_click_on_an_event_does_not_park_focus_on_the_grid() {
    let (sender, _receiver) = relm4::channel::<AppInput>();
    let surface = GridSurface::new(sender.clone());
    let scene = scrollable_scene();

    // `true` rather than the desktop's own setting: with accessibility off there are no hit
    // targets at all, and a test that quietly built none would pass while the defect still ships.
    rebuild_hits_with(&surface.hits, &scene, 800.0, &sender, true);

    let mut targets = 0;
    let mut child = surface.hits.first_child();
    while let Some(widget) = child {
        assert!(
            !widget.gets_focus_on_click(),
            "a hit target that takes focus on click drags the grid away on the next render"
        );
        assert!(
            widget.can_focus(),
            "a hit target still has to be reachable from the keyboard and a screen reader"
        );
        targets += 1;
        child = widget.next_sibling();
    }
    assert!(targets > 0, "the overlay drew no hit target to assert on");
}

/// A grid put on screen for the first time scrolls, and frames itself on the current hour.
///
/// Both halves are the same trap. An hour is as tall as the viewport says and the day is framed on
/// the hour it is, so height and offset are both settled from the scrolled window's own
/// adjustment; GTK emits those notifications from inside the viewport's size allocation, and
/// neither a height nor an offset asked for there reaches the screen. The calendar renders, holds
/// still under the wheel, and opens at midnight, until an unrelated render happens to resize it.
///
/// The clock is not fixed, so the framing is asserted as "it ran, and the grid moved with it":
/// the pending flag clears only once a real viewport produced an offset, and the grid's own
/// position is what says the offset was more than a number.
pub(crate) fn a_grid_shown_for_the_first_time_scrolls_and_frames_itself() {
    let (sender, _receiver) = relm4::channel::<AppInput>();
    let surface = GridSurface::new(sender);
    let mut scene = scrollable_scene();
    scene.timezone = "UTC".to_owned();
    *surface.scene.borrow_mut() = scene;
    let window = gtk::Window::new();
    window.set_default_size(900, 600);
    window.set_child(Some(&surface.root));
    window.present();
    surface.opened();

    let adjustment = surface.root.vadjustment();
    settle(|| !surface.framing.is_pending());
    settle(|| (drawn_top(&surface) + adjustment.value()).abs() < 1.0);
    let (page, upper, value, drawn) = (
        adjustment.page_size(),
        adjustment.upper(),
        adjustment.value(),
        drawn_top(&surface),
    );
    window.close();

    assert!(
        page > 0.0,
        "the grid was never given a viewport, so this proves nothing about one"
    );
    assert!(
        upper > page,
        "a day taller than its own viewport that cannot be scrolled: upper {upper}, page {page}"
    );
    assert!(
        !surface.framing.is_pending(),
        "the grid never framed itself on the current hour"
    );
    assert!(
        (drawn + value).abs() < 1.0,
        "the grid is drawn at {drawn} while the offset it holds says {}",
        -value
    );
}

/// Resizing the window keeps the hour on screen, rather than the offset that showed it.
///
/// An hour is as tall as the viewport says, so a taller window means a taller hour and the offset
/// that framed 09:25 is a different number afterwards. Worse, the day's own height is asked for
/// from an idle, so for a pass the scrolled window measures the *old* day against the new viewport
/// and clamps the offset into a range that does not exist: grow a window past twice its height and
/// the range collapses to nothing, which threw the calendar back to midnight.
pub(crate) fn a_resized_window_keeps_the_hour_the_reader_was_looking_at() {
    let (sender, _receiver) = relm4::channel::<AppInput>();
    let surface = GridSurface::new(sender);
    let mut scene = scrollable_scene();
    scene.timezone = "UTC".to_owned();
    *surface.scene.borrow_mut() = scene;
    let window = gtk::Window::new();
    window.set_default_size(900, 400);
    window.set_child(Some(&surface.root));
    window.present();
    surface.opened();
    let adjustment = surface.root.vadjustment();
    settle(|| !surface.framing.is_pending() && adjustment.upper() > adjustment.page_size());
    let before = centre_minutes(&surface);

    window.set_default_size(900, 1100);
    settle(|| adjustment.page_size() > 500.0);
    settle(|| (centre_minutes(&surface) - before).abs() < 2.0);
    let after = centre_minutes(&surface);
    let (page, upper, value) = (
        adjustment.page_size(),
        adjustment.upper(),
        adjustment.value(),
    );
    window.close();

    assert!(
        page > 500.0,
        "the window never grew, so this proves nothing about a resize"
    );
    assert!(
        value <= upper - page + 1.0,
        "the offset is outside the content it addresses: {value} of {}",
        upper - page
    );
    assert!(
        (after - before).abs() < 2.0,
        "the grid was showing minute {before} and came back at {after}"
    );
}

/// The minute in the middle of the viewport, read back off the grid the way a reader sees it.
///
/// Its own arithmetic on purpose, rather than the framing's: an oracle that calls the code under
/// test agrees with it by construction, including when both are wrong.
fn centre_minutes(surface: &GridSurface) -> f64 {
    let adjustment = surface.root.vadjustment();
    let scene = surface.scene.borrow();
    (adjustment.value() + adjustment.page_size() / 2.0 - scene.content_top()) * 60.0
        / scene.hour_height
}

/// Where the grid is really drawn, in the scrolled window's own coordinates.
fn drawn_top(surface: &GridSurface) -> f64 {
    surface
        .drawing
        .compute_point(&surface.root, &gtk::graphene::Point::new(0.0, 0.0))
        .map_or(0.0, |point| f64::from(point.y()))
}

/// Runs the main loop until `ready` holds, or gives up.
///
/// Layout is frame-clock work, so a widget put on screen is measured, allocated and measured again
/// over several frames, and both the height the grid asks for and the offset it is framed at land
/// a frame after the viewport that decided them. Draining the pending sources once sees none of
/// that. The deadline is generous because it is only ever spent when the assertion is about to
/// fail: a run that works returns on the frame it holds.
fn settle(ready: impl Fn() -> bool) {
    let context = gtk::glib::MainContext::default();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    while std::time::Instant::now() < deadline {
        while context.iteration(false) {}
        if ready() {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}

#[test]
fn semantic_nodes_follow_the_desktop_or_an_explicit_atspi_session() {
    assert!(semantic_nodes_enabled(true, None));
    assert!(semantic_nodes_enabled(false, Some("atspi")));
    assert!(!semantic_nodes_enabled(false, None));
}
