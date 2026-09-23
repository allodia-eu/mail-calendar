//! Drawn time-grid widget and its manually materialized AT-SPI overlay.

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use adw::prelude::*;
use gtk::accessible::Property as AccessibleProperty;

use super::{super::AppInput, model::CalendarModel};
use crate::l10n;

mod create;
mod draw;
mod geometry;
mod hits;
#[cfg(feature = "dev-harness")]
mod perf;
mod scene;
mod scroll;

use hits::{Surface, install_event_click, install_hit_target_css, pixel_size, rebuild_hits};
use scene::GridScene;
use scroll::Framing;

/// The GTK shell around the grid's two Cairo surfaces, and semantic nodes from the same geometry.
///
/// The day names and the all-day banner are a surface of their own above the scrolled hours, so
/// they stay on screen at whatever hour the reader has scrolled to.
pub(super) struct GridSurface {
    pub(super) root: gtk::Box,
    scroller: gtk::ScrolledWindow,
    header: gtk::DrawingArea,
    header_hits: gtk::Fixed,
    drawing: gtk::DrawingArea,
    hits: gtk::Fixed,
    scene: Rc<RefCell<GridScene>>,
    framing: Rc<Framing>,
    /// Retained so enabling a screen reader while the app is open rebuilds the semantic overlay.
    _accessibility_settings: gtk::gio::Settings,
    #[cfg(feature = "dev-harness")]
    perf_started: Rc<Cell<bool>>,
    sender: relm4::Sender<AppInput>,
}

impl GridSurface {
    pub(super) fn new(sender: relm4::Sender<AppInput>) -> Self {
        install_hit_target_css();
        let (drawing, hits, overlay) = layered_surface();
        let (header, header_hits, header_overlay) = layered_surface();
        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        let scroller = gtk::ScrolledWindow::new();
        scroller.set_hscrollbar_policy(gtk::PolicyType::Never);
        scroller.set_vexpand(true);
        scroller.set_child(Some(&overlay));
        root.append(&header_overlay);
        root.append(&scroller);
        scroller.update_property(&[AccessibleProperty::Label(l10n::nav_calendar())]);

        let scene = Rc::new(RefCell::new(GridScene::empty()));
        let framing = Rc::new(Framing::default());
        let draw_scene = Rc::clone(&scene);
        drawing.set_draw_func(move |_, context, width, _| {
            draw::draw_hours(&draw_scene.borrow(), context, f64::from(width));
        });
        let header_scene = Rc::clone(&scene);
        let header_width = drawing.clone();
        header.set_draw_func(move |_, context, _, _| {
            let width = f64::from(header_width.width());
            draw::draw_header(&header_scene.borrow(), context, width);
        });
        install_create_gesture(&hits, &drawing, &scene, &sender);
        install_event_click(&hits, &drawing, &scene, &sender, Surface::Hours);
        install_event_click(&header_hits, &drawing, &scene, &sender, Surface::Header);
        let resize_scene = Rc::clone(&scene);
        let resize_hits = hits.clone();
        let resize_header = header.clone();
        let resize_header_hits = header_hits.clone();
        let resize_sender = sender.clone();
        drawing.connect_resize(move |_, width, _| {
            let scene = resize_scene.borrow().clone();
            let width = f64::from(width);
            rebuild_hits(&resize_hits, &scene, width, &resize_sender, Surface::Hours);
            rebuild_hits(
                &resize_header_hits,
                &scene,
                width,
                &resize_sender,
                Surface::Header,
            );
            resize_header.queue_draw();
        });
        let viewport_drawing = drawing.clone();
        let viewport_hits = hits.clone();
        let viewport_scene = Rc::clone(&scene);
        let viewport_sender = sender.clone();
        let viewport_framing = Rc::clone(&framing);
        scroller
            .vadjustment()
            .connect_page_size_notify(move |adjustment| {
                fit_viewport(
                    &viewport_drawing,
                    &viewport_hits,
                    &viewport_scene,
                    adjustment.page_size(),
                    &viewport_sender,
                );
                settle_framing(adjustment, &viewport_scene, &viewport_framing);
            });
        let scroll_drawing = drawing.clone();
        let scroll_scene = Rc::clone(&scene);
        let scroll_framing = Rc::clone(&framing);
        // The painter culls to the viewport, and GTK moves a scrolled child without redrawing it.
        scroller
            .vadjustment()
            .connect_value_notify(move |adjustment| {
                scroll_scene
                    .borrow_mut()
                    .set_viewport_top(adjustment.value());
                scroll_drawing.queue_draw();
                scroll_framing.follow(adjustment, &scroll_scene);
            });
        let upper_scene = Rc::clone(&scene);
        let upper_framing = Rc::clone(&framing);
        scroller
            .vadjustment()
            .connect_upper_notify(move |adjustment| {
                settle_framing(adjustment, &upper_scene, &upper_framing);
            });
        let tick_drawing = drawing.clone();
        gtk::glib::timeout_add_seconds_local(60, move || {
            tick_drawing.queue_draw();
            gtk::glib::ControlFlow::Continue
        });
        let accessibility_settings = gtk::gio::Settings::new("org.gnome.desktop.interface");
        let accessibility_hits = hits.clone();
        let accessibility_header_hits = header_hits.clone();
        let accessibility_scene = Rc::clone(&scene);
        let accessibility_sender = sender.clone();
        let accessibility_drawing = drawing.clone();
        accessibility_settings.connect_changed(Some("toolkit-accessibility"), move |_, _| {
            let scene = accessibility_scene.borrow().clone();
            let width = f64::from(accessibility_drawing.width());
            let sender = &accessibility_sender;
            rebuild_hits(&accessibility_hits, &scene, width, sender, Surface::Hours);
            rebuild_hits(
                &accessibility_header_hits,
                &scene,
                width,
                sender,
                Surface::Header,
            );
        });
        Self {
            root,
            scroller,
            header,
            header_hits,
            drawing,
            hits,
            scene,
            framing,
            _accessibility_settings: accessibility_settings,
            #[cfg(feature = "dev-harness")]
            perf_started: Rc::new(Cell::new(false)),
            sender,
        }
    }

    pub(super) fn render(&self, model: &CalendarModel, dark: bool) {
        let drag = self.scene.borrow_mut().drag.take();
        let mut scene = GridScene::from_model(model, dark);
        scene.drag = drag;
        let adjustment = self.scroller.vadjustment();
        scene.set_viewport_top(adjustment.value());
        let header_height = pixel_size(scene.header_height().ceil());
        *self.scene.borrow_mut() = scene;
        self.header.set_content_height(header_height);
        self.header_hits.set_size_request(-1, header_height);
        let width = f64::from(self.drawing.width());
        if width > 0.0 {
            let scene = self.scene.borrow().clone();
            rebuild_hits(
                &self.header_hits,
                &scene,
                width,
                &self.sender,
                Surface::Header,
            );
        }
        self.header.queue_draw();
        fit_viewport(
            &self.drawing,
            &self.hits,
            &self.scene,
            adjustment.page_size(),
            &self.sender,
        );
        settle_framing(&adjustment, &self.scene, &self.framing);
        #[cfg(feature = "dev-harness")]
        perf::start_if_requested(
            &self.drawing,
            &adjustment,
            &self.scene,
            &self.perf_started,
            hits::semantic_nodes_active(),
        );
    }

    pub(super) fn opened(&self) {
        self.framing.open();
        settle_framing(&self.scroller.vadjustment(), &self.scene, &self.framing);
    }
}

/// A drawing with a hit plane laid over it, and the overlay that holds the two.
fn layered_surface() -> (gtk::DrawingArea, gtk::Fixed, gtk::Overlay) {
    let drawing = gtk::DrawingArea::new();
    drawing.set_hexpand(true);
    let hits = gtk::Fixed::new();
    hits.set_hexpand(true);
    hits.set_can_target(true);
    let overlay = gtk::Overlay::new();
    overlay.set_child(Some(&drawing));
    overlay.add_overlay(&hits);
    (drawing, hits, overlay)
}

/// Settles the grid's vertical position, once GTK is out of the layout pass that moved it.
///
/// Two of the callers reach this from the scrolled window's own adjustment, and GTK emits those
/// notifications from inside the viewport's size allocation, where an offset set on the adjustment
/// moves nothing: the viewport has already placed the grid for this frame, so it asks to be
/// allocated again, and that request goes the way `request_height`'s does. What is left is an
/// adjustment holding an offset the grid is not drawn at, which is worse than never framing it at
/// all: every later render preserves that offset, so the day reads as scrolled somewhere it never
/// went.
fn settle_framing(
    adjustment: &gtk::Adjustment,
    scene: &Rc<RefCell<GridScene>>,
    framing: &Rc<Framing>,
) {
    if !framing.owes(adjustment) {
        return;
    }
    let adjustment = adjustment.clone();
    let scene = Rc::clone(scene);
    let framing = Rc::clone(framing);
    gtk::glib::idle_add_local_once(move || framing.settle(&adjustment, &scene));
}

fn install_create_gesture(
    hits: &gtk::Fixed,
    drawing: &gtk::DrawingArea,
    scene: &Rc<RefCell<GridScene>>,
    sender: &relm4::Sender<AppInput>,
) {
    let gesture = gtk::GestureDrag::new();
    gesture.set_button(gtk::gdk::BUTTON_PRIMARY);
    gesture.set_propagation_phase(gtk::PropagationPhase::Capture);
    let cancelled = Rc::new(Cell::new(false));

    let begin_scene = Rc::clone(scene);
    let begin_drawing = drawing.clone();
    let begin_cancelled = Rc::clone(&cancelled);
    gesture.connect_drag_begin(move |gesture, x, y| {
        begin_cancelled.set(false);
        if begin_scene
            .borrow_mut()
            .begin_create(x, y, f64::from(begin_drawing.width()))
        {
            gesture.set_state(gtk::EventSequenceState::Claimed);
            begin_drawing.queue_draw();
        } else {
            gesture.set_state(gtk::EventSequenceState::Denied);
        }
    });

    let update_scene = Rc::clone(scene);
    let update_drawing = drawing.clone();
    let update_cancelled = Rc::clone(&cancelled);
    gesture.connect_drag_update(move |gesture, _offset_x, offset_y| {
        if update_cancelled.get() {
            return;
        }
        let Some((_, start_y)) = gesture.start_point() else {
            return;
        };
        update_scene
            .borrow_mut()
            .update_create(start_y + offset_y, f64::from(update_drawing.width()));
        update_drawing.queue_draw();
    });

    let end_scene = Rc::clone(scene);
    let end_drawing = drawing.clone();
    let end_sender = sender.clone();
    let end_cancelled = Rc::clone(&cancelled);
    gesture.connect_drag_end(move |gesture, _offset_x, offset_y| {
        if end_cancelled.replace(false) {
            return;
        }
        if let Some((_, start_y)) = gesture.start_point() {
            end_scene
                .borrow_mut()
                .update_create(start_y + offset_y, f64::from(end_drawing.width()));
        }
        let slot = end_scene.borrow_mut().finish_create();
        end_drawing.queue_draw();
        if let Some(slot) = slot {
            end_sender.emit(AppInput::BeginNewEventAt(slot));
        }
    });

    let cancel_scene = Rc::clone(scene);
    let cancel_drawing = drawing.clone();
    gesture.connect_cancel(move |_, _| {
        cancelled.set(true);
        cancel_scene.borrow_mut().cancel_create();
        cancel_drawing.queue_draw();
    });
    hits.add_controller(gesture);
}

fn fit_viewport(
    drawing: &gtk::DrawingArea,
    hits: &gtk::Fixed,
    scene: &Rc<RefCell<GridScene>>,
    viewport_height: f64,
    sender: &relm4::Sender<AppInput>,
) {
    let height = {
        let mut scene = scene.borrow_mut();
        if viewport_height > 0.0 {
            scene.fit_viewport(viewport_height);
        }
        pixel_size(scene.height().ceil())
    };
    request_height(drawing, hits, height);
    let width = f64::from(drawing.width());
    if width > 0.0 {
        let scene = scene.borrow().clone();
        rebuild_hits(hits, &scene, width, sender, Surface::Hours);
    }
    drawing.queue_draw();
}

/// Asks for the day's own height, once GTK is out of the layout pass that decided it.
///
/// An hour is as tall as the viewport says, so the height follows a page size GTK announces from
/// inside the viewport's size allocation, and a height requested there is measured too late: the
/// grid is allocated again at the height it already had, nothing measures it a second time, and
/// the scrolled window's `upper` never leaves the page size. The day then has no scroll range at
/// all and no centre to frame itself on, and stays that way until an unrelated render changes the
/// height for its own reasons. An idle lands after the frame, where a queued resize is honoured.
fn request_height(drawing: &gtk::DrawingArea, hits: &gtk::Fixed, height: i32) {
    let drawing = drawing.clone();
    let hits = hits.clone();
    gtk::glib::idle_add_local_once(move || {
        drawing.set_content_height(height);
        hits.set_size_request(-1, height);
    });
}

#[cfg(test)]
#[path = "grid_widget_tests.rs"]
pub(crate) mod widget_tests;
