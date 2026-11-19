//! The host for a multi-step flow (`docs/ai.md`, "The flow host"): an `AdwNavigationView` with a
//! page per step, drawn over the Settings window's detail, its header bar standing in for the
//! window's own while it is up.
//!
//! It holds no step, index or rule of its own. Its stack is the record's: the trail of steps under
//! the one on screen, then that step, so a step forward is a push, Back is a pop, and anything else
//! is the stack set to what the record says. The header's back button is Back while the step
//! offers it; otherwise the header's start holds the one way out, Cancel. The primary is a
//! suggested action at the end, beside the window's own controls.

use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::Rc,
};

use adw::prelude::*;
use mailcal_bindings::ControlState;

use super::controls;

/// A press on one of a step's buttons.
pub(crate) type Pressed = Rc<dyn Fn()>;

/// A step's primary action: Next, Learn, Stop, a remedy, Save.
pub(crate) struct FlowAction {
    pub(crate) label: String,
    pub(crate) control: ControlState,
    /// Drawn as the suggested action: a step forward rather than Stop.
    pub(crate) suggested: bool,
    pub(crate) pressed: Pressed,
}

/// The way out of a step that offers no Back: Cancel, or Close.
pub(crate) struct FlowLeave {
    pub(crate) label: String,
    pub(crate) control: ControlState,
}

/// One step as the record has it.
pub(crate) struct FlowStep {
    /// What the step is, as the trail names it.
    pub(crate) tag: &'static str,
    pub(crate) title: String,
    /// What the header shows in the title's place, when the step has a control there.
    pub(crate) title_widget: Option<gtk::Widget>,
    pub(crate) content: gtk::Widget,
    /// Back to the step under it on the trail, drawn as the header's back button.
    pub(crate) back: Option<ControlState>,
    /// The way out where there is no Back.
    pub(crate) leave: FlowLeave,
    /// A back button of the step's own (a carousel's), drawn in the way out's place while it is
    /// shown.
    pub(crate) own_back: Option<gtk::Button>,
    pub(crate) primary: Option<FlowAction>,
    /// Drawn below the content: a page indicator, for a step that has pages.
    pub(crate) footer: Option<gtk::Widget>,
}

/// A step under the one on screen: its tag and its title, which the back button names.
pub(crate) struct FlowTrailStep {
    pub(crate) tag: &'static str,
    pub(crate) title: String,
}

/// The host. Its owner keeps it for as long as it is on screen.
pub(crate) struct FlowHost {
    view: adw::NavigationView,
    /// Set while the host moves the stack itself, which is not the person's Back.
    moving: Cell<bool>,
    back: RefCell<Option<Pressed>>,
    leave: RefCell<Option<Pressed>>,
    /// Each page's frame as drawn last, so the widgets a step keeps between draws (a carousel, its
    /// dots, a title control) are taken out of the old frame before the new one holds them.
    frames: RefCell<HashMap<&'static str, Frame>>,
}

struct Frame {
    toolbar: adw::ToolbarView,
    header: adw::HeaderBar,
    own_back: Option<gtk::Button>,
    footer: Option<gtk::Widget>,
}

impl Frame {
    fn release(&self) {
        if let Some(own_back) = &self.own_back {
            self.header.remove(own_back);
        }
        if let Some(footer) = &self.footer {
            self.toolbar.remove(footer);
        }
        self.header.set_title_widget(None::<&gtk::Widget>);
        self.toolbar.set_content(None::<&gtk::Widget>);
    }
}

impl FlowHost {
    /// A host that tells `back` when the person goes back and `leave` when they take the way out.
    pub(crate) fn new(back: Pressed, leave: Pressed) -> Rc<Self> {
        let view = adw::NavigationView::new();
        view.set_hexpand(true);
        view.set_vexpand(true);
        let host = Rc::new(Self {
            view,
            moving: Cell::new(false),
            back: RefCell::new(Some(back)),
            leave: RefCell::new(Some(leave)),
            frames: RefCell::default(),
        });
        let weak = Rc::downgrade(&host);
        host.view.connect_popped(move |_, _| {
            let Some(host) = weak.upgrade() else {
                return;
            };
            if !host.moving.get() {
                let back = host.back.borrow().clone();
                if let Some(back) = back {
                    back();
                }
            }
        });
        host
    }

    pub(crate) fn widget(&self) -> &adw::NavigationView {
        &self.view
    }

    /// Draws `step` on top of `trail`: the stack becomes the trail's pages and the step's, by a
    /// push when the step is one further, a pop when it is below, and otherwise as a whole.
    pub(crate) fn show(self: &Rc<Self>, trail: &[FlowTrailStep], step: FlowStep) {
        let wanted = trail
            .iter()
            .map(|below| below.tag)
            .chain(std::iter::once(step.tag))
            .map(str::to_owned)
            .collect::<Vec<_>>();
        let stack = self.stack();
        let tags = stack.iter().map(page_tag).collect::<Vec<_>>();
        if tags != wanted {
            self.moving.set(true);
            if tags.starts_with(&wanted) {
                self.view.pop_to_tag(step.tag);
            } else if wanted.starts_with(&tags)
                && wanted.len() == tags.len() + 1
                && !tags.is_empty()
            {
                self.view.push(&page(step.tag, &step.title));
            } else {
                let pages = trail
                    .iter()
                    .map(|below| (below.tag, below.title.as_str()))
                    .chain(std::iter::once((step.tag, step.title.as_str())))
                    .map(|(tag, title)| {
                        stack
                            .iter()
                            .find(|kept| page_tag(kept) == tag)
                            .cloned()
                            .unwrap_or_else(|| page(tag, title))
                    })
                    .collect::<Vec<_>>();
                self.view.replace(&pages);
            }
            self.moving.set(false);
        }
        let Some(page) = self.view.find_page(step.tag) else {
            return;
        };
        page.set_title(&step.title);
        page.set_can_pop(step.back.is_some_and(|back| back.enabled));
        if let Some(old) = self.frames.borrow_mut().remove(step.tag) {
            old.release();
        }
        let tag = step.tag;
        let frame = self.frame(step);
        page.set_child(Some(&frame.toolbar));
        self.frames.borrow_mut().insert(tag, frame);
    }

    /// Takes the host off screen: nothing it does afterwards reaches its owner.
    pub(crate) fn dismiss(&self) {
        self.back.replace(None);
        self.leave.replace(None);
    }

    pub(crate) fn stack(&self) -> Vec<adw::NavigationPage> {
        let model = self.view.navigation_stack();
        (0..model.n_items())
            .filter_map(|position| model.item(position).and_downcast::<adw::NavigationPage>())
            .collect()
    }

    /// The step's toolbar: the way out or Back at the start, the primary at the end, the content
    /// between and what the step draws under it.
    fn frame(self: &Rc<Self>, step: FlowStep) -> Frame {
        let toolbar = adw::ToolbarView::new();
        let header = adw::HeaderBar::new();
        // One way out of a flow, its own at the start: the window's close stays with the window.
        header.set_show_start_title_buttons(false);
        header.set_show_end_title_buttons(false);
        let title = step
            .title_widget
            .unwrap_or_else(|| adw::WindowTitle::new(&step.title, "").upcast::<gtk::Widget>());
        header.set_title_widget(Some(&title));
        let has_back = step.back.is_some_and(|back| back.enabled);
        if let Some(own_back) = &step.own_back {
            header.pack_start(own_back);
        }
        let own_back_shown = step.own_back.as_ref().is_some_and(WidgetExt::is_visible);
        if !has_back && !own_back_shown {
            let weak = Rc::downgrade(self);
            let leave = controls::button(&step.leave.label, step.leave.control, false, move || {
                let leave = weak.upgrade().and_then(|host| host.leave.borrow().clone());
                if let Some(leave) = leave {
                    leave();
                }
            });
            wraps(&leave);
            header.pack_start(&leave);
        }
        if let Some(primary) = step.primary {
            let pressed = primary.pressed;
            let button = controls::button(&primary.label, primary.control, false, move || {
                pressed();
            });
            if primary.suggested {
                button.add_css_class("suggested-action");
            }
            wraps(&button);
            header.pack_end(&button);
        }
        toolbar.add_top_bar(&header);
        toolbar.set_content(Some(&step.content));
        if let Some(footer) = &step.footer {
            toolbar.add_bottom_bar(footer);
        }
        Frame {
            toolbar,
            header,
            own_back: step.own_back,
            footer: step.footer,
        }
    }
}

fn page(tag: &str, title: &str) -> adw::NavigationPage {
    adw::NavigationPage::with_tag(&adw::Bin::new(), title, tag)
}

fn page_tag(page: &adw::NavigationPage) -> String {
    page.tag().map(|tag| tag.to_string()).unwrap_or_default()
}

/// Lets a button's label wrap between words at a large text size, never inside one.
pub(crate) fn wraps(button: &impl IsA<gtk::Widget>) {
    let mut child = button.as_ref().first_child();
    while let Some(widget) = child {
        if let Some(label) = widget.downcast_ref::<gtk::Label>() {
            label.set_wrap(true);
            label.set_wrap_mode(gtk::pango::WrapMode::Word);
            label.set_justify(gtk::Justification::Center);
        } else {
            wraps(&widget);
        }
        child = widget.next_sibling();
    }
}

#[cfg(test)]
#[path = "flow_host_tests.rs"]
pub(crate) mod tests;
