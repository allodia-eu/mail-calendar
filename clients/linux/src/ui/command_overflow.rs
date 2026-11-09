//! A row of controls that keeps as many as fit and moves the rest, from the end and in order, into
//! a ⋯ menu at the row's end (`docs/command-bar.md`, rule 4). What fits is measured, never a width
//! written here: a longer language and a larger text size each move the answer.
//!
//! The toolkit measures, and this file only listens. The controls ride an overlay that does not
//! take part in measuring, so a narrow window may be narrower than all of them together; beneath
//! them is a drawing area that takes whatever width the row is given and says so in its `resize`
//! signal. Each control sits in a revealer, which takes no width while shut but leaves the
//! control inside it measurable, so the answer can be taken again after a label changes.

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use adw::prelude::*;
use gtk::accessible::Property as AccessibleProperty;

use crate::{l10n, ui::icons};

/// Space between two controls, and between a divider and the controls beside it.
const SPACING: i32 = 4;

/// One control on the row.
pub(crate) struct OverflowEntry {
    /// What stands on the row.
    pub(crate) widget: gtk::Widget,
    /// Which group it belongs to; a divider goes before the first control of each later group.
    pub(crate) section: usize,
    /// What it reads in the menu once it has moved there.
    pub(crate) label: String,
    /// The detailed action the menu item activates.
    pub(crate) action: String,
}

/// An item that always stands in the menu, past whatever moved there.
pub(crate) struct OverflowExtra {
    pub(crate) label: String,
    pub(crate) action: String,
}

struct Placed {
    revealer: gtk::Revealer,
    section: usize,
    label: String,
    action: String,
}

struct Fit {
    placed: RefCell<Vec<Placed>>,
    extras: RefCell<Vec<OverflowExtra>>,
    /// The width the sensor was last given; `None` until the row is first allocated.
    sensed: Cell<Option<i32>>,
    /// How many controls stand on the row.
    shown: Cell<usize>,
    more: gtk::MenuButton,
    more_slot: gtk::Revealer,
}

pub(crate) struct OverflowRow {
    root: gtk::Overlay,
    items: gtk::Box,
    fit: Rc<Fit>,
}

impl OverflowRow {
    pub(crate) fn new() -> Self {
        let more = gtk::MenuButton::new();
        more.set_icon_name(icons::MORE);
        more.add_css_class("flat");
        more.set_tooltip_text(Some(l10n::a11y_more_actions()));
        more.update_property(&[AccessibleProperty::Label(l10n::a11y_more_actions())]);
        more.set_valign(gtk::Align::Center);
        more.set_margin_start(SPACING);
        // In a revealer rather than hidden, for the reason each control is: a hidden widget
        // measures nothing, and the fit needs the button's width before it decides to show it.
        let more_slot = gtk::Revealer::new();
        more_slot.set_transition_type(gtk::RevealerTransitionType::None);
        more_slot.set_reveal_child(false);
        more_slot.set_child(Some(&more));

        let sensor = gtk::DrawingArea::new();
        sensor.set_hexpand(true);
        sensor.set_can_target(false);
        // The row's height is the ⋯ button's, which is a button's like every control above it.
        let base = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        base.append(&sensor);
        base.append(&more_slot);

        let items = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        items.set_halign(gtk::Align::Start);
        items.set_valign(gtk::Align::Center);
        let root = gtk::Overlay::new();
        root.set_child(Some(&base));
        root.add_overlay(&items);
        root.set_clip_overlay(&items, true);
        root.set_hexpand(true);

        let fit = Rc::new(Fit {
            placed: RefCell::default(),
            extras: RefCell::default(),
            sensed: Cell::new(None),
            shown: Cell::new(0),
            more: more.clone(),
            more_slot,
        });
        let sensing = Rc::clone(&fit);
        sensor.connect_resize(move |_, width, _| {
            sensing.sensed.set(Some(width));
            sensing.refit();
        });
        // A popover of buttons rather than a menu model: a model's items reach the accessibility
        // bus with no name, so a screen reader would read a menu of blanks. The row menus are
        // built the same way.
        let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
        content.set_margin_top(6);
        content.set_margin_bottom(6);
        content.set_margin_start(6);
        content.set_margin_end(6);
        let popover = gtk::Popover::new();
        popover.set_child(Some(&content));
        more.set_popover(Some(&popover));
        let filling = Rc::downgrade(&fit);
        more.set_create_popup_func(move |_| {
            if let Some(fit) = filling.upgrade() {
                fill(&content, &popover, &fit.groups());
            }
        });
        Self { root, items, fit }
    }

    pub(crate) fn widget(&self) -> &gtk::Overlay {
        &self.root
    }

    /// Replaces every control, in order.
    pub(crate) fn set_entries(&self, entries: Vec<OverflowEntry>, extras: Vec<OverflowExtra>) {
        while let Some(child) = self.items.first_child() {
            self.items.remove(&child);
        }
        let mut placed = Vec::with_capacity(entries.len());
        let mut previous = None;
        for entry in entries {
            let content = gtk::Box::new(gtk::Orientation::Horizontal, SPACING);
            content.set_margin_end(SPACING);
            if previous.is_some_and(|section| section != entry.section) {
                let divider = gtk::Separator::new(gtk::Orientation::Vertical);
                divider.set_margin_top(6);
                divider.set_margin_bottom(6);
                content.append(&divider);
            }
            previous = Some(entry.section);
            content.append(&entry.widget);
            let revealer = gtk::Revealer::new();
            revealer.set_transition_type(gtk::RevealerTransitionType::None);
            revealer.set_reveal_child(true);
            revealer.set_child(Some(&content));
            self.items.append(&revealer);
            placed.push(Placed {
                revealer,
                section: entry.section,
                label: entry.label,
                action: entry.action,
            });
        }
        self.fit.placed.replace(placed);
        self.fit.extras.replace(extras);
        self.fit.refit();
    }

    /// Renames the control at `index` in the menu, after its label on the row changed.
    pub(crate) fn relabel(&self, index: usize, label: &str) {
        if let Some(placed) = self.fit.placed.borrow_mut().get_mut(index) {
            label.clone_into(&mut placed.label);
        }
    }

    /// Takes the answer again at the last width, after a label changed what the controls need.
    pub(crate) fn refit(&self) {
        self.fit.refit();
    }

    /// How many controls stand on the row; the rest are in the menu.
    #[cfg(test)]
    pub(crate) fn shown(&self) -> usize {
        self.fit.shown.get()
    }

    /// Whether the ⋯ button stands at the row's end.
    #[cfg(test)]
    pub(crate) fn more_shown(&self) -> bool {
        self.fit.more_slot.reveals_child()
    }

    /// The labels the ⋯ menu would hold now, grouped as it draws them.
    #[cfg(test)]
    pub(crate) fn menu(&self) -> Vec<Vec<String>> {
        self.fit
            .groups()
            .into_iter()
            .map(|group| group.into_iter().map(|(label, _)| label).collect())
            .collect()
    }

    /// Answers as if the row had been given `width`, which is what the sensor reports.
    #[cfg(test)]
    pub(crate) fn sense(&self, width: i32) {
        self.fit.sensed.set(Some(width));
        self.fit.refit();
    }
}

impl Fit {
    /// Stands on the row every control that fits, from the start, and moves the rest.
    fn refit(&self) {
        let placed = self.placed.borrow();
        let has_extras = !self.extras.borrow().is_empty();
        let Some(sensed) = self.sensed.get() else {
            // Not allocated yet: everything stands, and the first allocation decides.
            self.show(&placed, placed.len(), has_extras);
            return;
        };
        let more_width = natural_width(&self.more.clone().upcast());
        // The width the controls and the ⋯ button share, whichever of them stands now: the
        // sensor is given what the button leaves.
        let total = sensed
            + if self.more_slot.reveals_child() {
                more_width
            } else {
                0
            };
        let widths: Vec<i32> = placed
            .iter()
            .map(|placed| {
                placed
                    .revealer
                    .child()
                    .map_or(0, |content| natural_width(&content))
            })
            .collect();
        let all: i32 = widths.iter().sum();
        let shown = if !has_extras && all <= total {
            placed.len()
        } else {
            let budget = total - more_width;
            let mut used = 0;
            widths
                .iter()
                .take_while(|width| {
                    used += **width;
                    used <= budget
                })
                .count()
        };
        self.show(&placed, shown, has_extras);
    }

    fn show(&self, placed: &[Placed], shown: usize, has_extras: bool) {
        for (index, placed) in placed.iter().enumerate() {
            placed.revealer.set_reveal_child(index < shown);
        }
        self.shown.set(shown);
        self.more_slot
            .set_reveal_child(has_extras || shown < placed.len());
    }

    /// What moved, in bar order and grouped as on the bar, then the extras past a separator: each
    /// item's label and the detailed action it runs.
    fn groups(&self) -> Vec<Vec<(String, String)>> {
        let mut groups: Vec<Vec<(String, String)>> = Vec::new();
        let mut section = None;
        for placed in self.placed.borrow().iter().skip(self.shown.get()) {
            if section != Some(placed.section) {
                groups.push(Vec::new());
                section = Some(placed.section);
            }
            if let Some(group) = groups.last_mut() {
                group.push((placed.label.clone(), placed.action.clone()));
            }
        }
        let extras: Vec<_> = self
            .extras
            .borrow()
            .iter()
            .map(|extra| (extra.label.clone(), extra.action.clone()))
            .collect();
        if !extras.is_empty() {
            groups.push(extras);
        }
        groups
    }
}

/// Draws `groups` into the popover's `content`, a separator between each two. Each item is a
/// button bound to its action, so it responds exactly when the action does.
fn fill(content: &gtk::Box, popover: &gtk::Popover, groups: &[Vec<(String, String)>]) {
    while let Some(child) = content.first_child() {
        content.remove(&child);
    }
    for (index, group) in groups.iter().enumerate() {
        if index > 0 {
            content.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
        }
        for (label, action) in group {
            let text = gtk::Label::new(Some(label));
            text.set_xalign(0.0);
            let button = gtk::Button::new();
            button.set_child(Some(&text));
            button.add_css_class("flat");
            button.update_property(&[AccessibleProperty::Label(label)]);
            button.set_action_name(Some(action));
            let closing = popover.clone();
            button.connect_clicked(move |_| closing.popdown());
            content.append(&button);
        }
    }
}

fn natural_width(widget: &gtk::Widget) -> i32 {
    widget.measure(gtk::Orientation::Horizontal, -1).1
}
