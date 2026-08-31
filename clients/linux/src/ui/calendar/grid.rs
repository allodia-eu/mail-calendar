//! Drawn time-grid widget and its manually materialized AT-SPI overlay.

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use adw::prelude::*;
use gtk::accessible::Property as AccessibleProperty;

use super::{super::AppInput, date::now_in, model::CalendarModel};
use crate::l10n;

mod create;
mod draw;
#[cfg(feature = "dev-harness")]
mod perf;
mod scene;
mod scroll;

use scene::{GUTTER, GridScene, HEADING_HEIGHT};

/// The GTK shell around one Cairo surface and semantic nodes from the same geometry.
pub(super) struct GridSurface {
    pub(super) root: gtk::ScrolledWindow,
    drawing: gtk::DrawingArea,
    hits: gtk::Fixed,
    scene: Rc<RefCell<GridScene>>,
    recentre_pending: Rc<Cell<bool>>,
    /// Retained so enabling a screen reader while the app is open rebuilds the semantic overlay.
    _accessibility_settings: gtk::gio::Settings,
    #[cfg(feature = "dev-harness")]
    perf_started: Rc<Cell<bool>>,
    sender: relm4::Sender<AppInput>,
}

impl GridSurface {
    pub(super) fn new(sender: relm4::Sender<AppInput>) -> Self {
        install_hit_target_css();
        let drawing = gtk::DrawingArea::new();
        drawing.set_hexpand(true);
        let hits = gtk::Fixed::new();
        hits.set_hexpand(true);
        hits.set_can_target(true);
        let overlay = gtk::Overlay::new();
        overlay.set_child(Some(&drawing));
        overlay.add_overlay(&hits);
        let root = gtk::ScrolledWindow::new();
        root.set_hscrollbar_policy(gtk::PolicyType::Never);
        root.set_child(Some(&overlay));
        root.update_property(&[AccessibleProperty::Label(l10n::nav_calendar())]);

        let scene = Rc::new(RefCell::new(GridScene::empty()));
        let recentre_pending = Rc::new(Cell::new(false));
        let draw_scene = Rc::clone(&scene);
        drawing.set_draw_func(move |_, context, width, _| {
            draw::draw(&draw_scene.borrow(), context, f64::from(width));
        });
        install_create_gesture(&hits, &drawing, &scene, &sender);
        install_event_click(&hits, &drawing, &scene, &sender);
        let resize_scene = Rc::clone(&scene);
        let resize_hits = hits.clone();
        let resize_sender = sender.clone();
        drawing.connect_resize(move |_, width, _| {
            let scene = resize_scene.borrow().clone();
            rebuild_hits(&resize_hits, &scene, f64::from(width), &resize_sender);
        });
        let viewport_drawing = drawing.clone();
        let viewport_hits = hits.clone();
        let viewport_scene = Rc::clone(&scene);
        let viewport_sender = sender.clone();
        let viewport_recentre = Rc::clone(&recentre_pending);
        root.vadjustment()
            .connect_page_size_notify(move |adjustment| {
                fit_viewport(
                    &viewport_drawing,
                    &viewport_hits,
                    &viewport_scene,
                    adjustment.page_size(),
                    &viewport_sender,
                );
                apply_recentre(adjustment, &viewport_scene, &viewport_recentre);
            });
        let scroll_scene = Rc::clone(&scene);
        root.vadjustment().connect_value_notify(move |adjustment| {
            scroll_scene
                .borrow_mut()
                .set_viewport_top(adjustment.value());
        });
        let upper_scene = Rc::clone(&scene);
        let upper_recentre = Rc::clone(&recentre_pending);
        root.vadjustment().connect_upper_notify(move |adjustment| {
            apply_recentre(adjustment, &upper_scene, &upper_recentre);
        });
        let tick_drawing = drawing.clone();
        gtk::glib::timeout_add_seconds_local(60, move || {
            tick_drawing.queue_draw();
            gtk::glib::ControlFlow::Continue
        });
        let accessibility_settings = gtk::gio::Settings::new("org.gnome.desktop.interface");
        let accessibility_hits = hits.clone();
        let accessibility_scene = Rc::clone(&scene);
        let accessibility_sender = sender.clone();
        let accessibility_drawing = drawing.clone();
        accessibility_settings.connect_changed(Some("toolkit-accessibility"), move |_, _| {
            let scene = accessibility_scene.borrow().clone();
            rebuild_hits(
                &accessibility_hits,
                &scene,
                f64::from(accessibility_drawing.width()),
                &accessibility_sender,
            );
        });
        Self {
            root,
            drawing,
            hits,
            scene,
            recentre_pending,
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
        scene.set_viewport_top(self.root.vadjustment().value());
        *self.scene.borrow_mut() = scene;
        fit_viewport(
            &self.drawing,
            &self.hits,
            &self.scene,
            self.root.vadjustment().page_size(),
            &self.sender,
        );
        apply_recentre(
            &self.root.vadjustment(),
            &self.scene,
            &self.recentre_pending,
        );
        #[cfg(feature = "dev-harness")]
        perf::start_if_requested(
            &self.drawing,
            &self.root.vadjustment(),
            &self.scene,
            &self.perf_started,
            semantic_nodes_active(),
        );
    }

    pub(super) fn opened(&self) {
        self.recentre_pending.set(true);
        apply_recentre(
            &self.root.vadjustment(),
            &self.scene,
            &self.recentre_pending,
        );
    }
}

fn apply_recentre(adjustment: &gtk::Adjustment, scene: &RefCell<GridScene>, pending: &Cell<bool>) {
    if !pending.get() {
        return;
    }
    let minutes = {
        let scene = scene.borrow();
        let Some((_, minutes)) = now_in(&scene.timezone) else {
            return;
        };
        minutes
    };
    apply_recentre_at(adjustment, scene, pending, minutes);
}

fn apply_recentre_at(
    adjustment: &gtk::Adjustment,
    scene: &RefCell<GridScene>,
    pending: &Cell<bool>,
    minutes: u32,
) {
    let value = {
        let scene = scene.borrow();
        scroll::centred_scroll_value(
            scene.content_top(),
            scene.hour_height,
            minutes,
            adjustment.page_size(),
            adjustment.upper(),
        )
    };
    let Some(value) = value else {
        return;
    };
    pending.set(false);
    adjustment.set_value(value);
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

fn install_event_click(
    hits: &gtk::Fixed,
    drawing: &gtk::DrawingArea,
    scene: &Rc<RefCell<GridScene>>,
    sender: &relm4::Sender<AppInput>,
) {
    let gesture = gtk::GestureClick::new();
    gesture.set_button(gtk::gdk::BUTTON_PRIMARY);
    let click_scene = Rc::clone(scene);
    let click_drawing = drawing.clone();
    let input = sender.clone();
    gesture.connect_released(move |_, presses, x, y| {
        if presses != 1 || semantic_nodes_active() {
            return;
        }
        let hit = click_scene
            .borrow()
            .geometry(f64::from(click_drawing.width()))
            .hits
            .into_iter()
            .find(|hit| {
                x >= hit.rect.x
                    && x < hit.rect.x + hit.rect.width
                    && y >= hit.rect.y
                    && y < hit.rect.y + hit.rect.height
            });
        if let Some(hit) = hit {
            if let Some(identity) = hit.identity {
                input.emit(AppInput::OpenCalendarEvent(identity));
            } else {
                input.emit(AppInput::ToggleAllDay);
            }
        }
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
    drawing.set_content_height(height);
    hits.set_size_request(-1, height);
    let width = f64::from(drawing.width());
    if width > 0.0 {
        let scene = scene.borrow().clone();
        rebuild_hits(hits, &scene, width, sender);
    }
    drawing.queue_draw();
}

fn rebuild_hits(
    fixed: &gtk::Fixed,
    scene: &GridScene,
    width: f64,
    sender: &relm4::Sender<AppInput>,
) {
    rebuild_hits_with(fixed, scene, width, sender, semantic_nodes_active());
}

fn rebuild_hits_with(
    fixed: &gtk::Fixed,
    scene: &GridScene,
    width: f64,
    sender: &relm4::Sender<AppInput>,
    semantic: bool,
) {
    while let Some(child) = fixed.first_child() {
        fixed.remove(&child);
    }
    if !semantic {
        return;
    }
    if let Some(status) = scene.semantic_status() {
        append_loading_node(fixed, width, &status);
        return;
    }
    for hit in scene.geometry(width).hits {
        let button = gtk::Button::new();
        button.add_css_class("calendar-hit-target");
        // Keyboard-reachable, but never focused *by the click itself*. Opening an event rebuilds
        // this overlay, which destroys the button the click landed on; GTK then moves focus off
        // the dying widget and the scrolled window animates to reveal whatever inherits it,
        // the first target, at the top of the day. The reader was hours further down.
        button.set_focus_on_click(false);
        button.set_tooltip_text(Some(&hit.spoken));
        button.update_property(&[AccessibleProperty::Label(&hit.spoken)]);
        button.set_size_request(
            pixel_size(hit.rect.width.max(1.0).round()),
            pixel_size(hit.rect.height.max(1.0).round()),
        );
        let input = sender.clone();
        if let Some(identity) = hit.identity {
            button.connect_clicked(move |_| {
                input.emit(AppInput::OpenCalendarEvent(identity.clone()));
            });
        } else {
            button.connect_clicked(move |_| input.emit(AppInput::ToggleAllDay));
        }
        fixed.put(&button, hit.rect.x, hit.rect.y);
    }
}

fn semantic_nodes_active() -> bool {
    semantic_nodes_enabled(
        gtk::gio::Settings::new("org.gnome.desktop.interface").boolean("toolkit-accessibility"),
        std::env::var("GTK_A11Y").ok().as_deref(),
    )
}

fn semantic_nodes_enabled(toolkit_accessibility: bool, requested_backend: Option<&str>) -> bool {
    toolkit_accessibility
        || requested_backend.is_some_and(|value| value.eq_ignore_ascii_case("atspi"))
}

fn append_loading_node(fixed: &gtk::Fixed, width: f64, status: &str) {
    let loading = gtk::Label::new(Some(status));
    loading.add_css_class("calendar-loading-target");
    loading.update_property(&[AccessibleProperty::Label(status)]);
    loading.set_size_request(pixel_size((width - GUTTER).max(1.0)), 48);
    fixed.put(&loading, GUTTER, HEADING_HEIGHT);
}

/// Calendar geometry is bounded to a few thousand pixels; clamp before the GTK integer boundary.
#[allow(clippy::cast_possible_truncation)]
fn pixel_size(value: f64) -> i32 {
    value.clamp(1.0, f64::from(i32::MAX)).round() as i32
}

fn install_hit_target_css() {
    static INSTALLED: std::sync::Once = std::sync::Once::new();
    INSTALLED.call_once(|| {
        let provider = gtk::CssProvider::new();
        provider.load_from_string(
            ".calendar-hit-target { background: transparent; color: transparent; border-color: transparent; box-shadow: none; padding: 0; min-width: 0; min-height: 0; } .calendar-loading-target { color: transparent; }",
        );
        if let Some(display) = gtk::gdk::Display::default() {
            gtk::style_context_add_provider_for_display(
                &display,
                &provider,
                gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
            );
        }
    });
}

#[cfg(test)]
#[path = "grid_widget_tests.rs"]
pub(crate) mod widget_tests;
