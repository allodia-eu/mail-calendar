//! The host for a multi-step flow (`docs/ai.md`, "The flow host"): an `AdwNavigationView` with a
//! page per step, drawn inside the Settings window's detail rather than as a second toplevel.
//!
//! It holds no step, index or rule of its own. A caller hands it the step the core's record is on,
//! with the controls that record gives, and the host only decides how the change looks: a step
//! the person moved forward to is pushed, a step already on the stack is popped back to, and any
//! other replaces the page on top. The header bar's back button is Back, and is drawn only while
//! the step offers one; Cancel sits at the start and the primary action at the end.

use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::Rc,
};

use adw::prelude::*;
use mailcal_bindings::ControlState;

use super::controls;
use crate::l10n;

/// The tag of the page that stands below a first step offering Back, so the header bar draws its
/// back button there too.
const BELOW: &str = "flow-below";

/// A press on one of a step's buttons.
pub(crate) type Pressed = Rc<dyn Fn()>;

/// A step's primary action: Next, Learn, Stop, a remedy.
pub(crate) struct FlowAction {
    pub(crate) label: String,
    pub(crate) control: ControlState,
    /// Drawn as the suggested action: a step forward rather than Stop.
    pub(crate) suggested: bool,
    /// Moves the flow forward, so the step it leads to is pushed rather than swapped in.
    pub(crate) advances: bool,
    pub(crate) pressed: Pressed,
}

/// One step as the record has it.
pub(crate) struct FlowStep {
    /// What the step is, so the host can tell a new step from the one on screen.
    pub(crate) tag: &'static str,
    pub(crate) title: String,
    /// What the header shows in the title's place, when the step has a control there.
    pub(crate) title_widget: Option<gtk::Widget>,
    pub(crate) content: gtk::Widget,
    pub(crate) back: Option<ControlState>,
    pub(crate) primary: Option<FlowAction>,
    pub(crate) close: ControlState,
    /// Drawn below the content: a page indicator, for a step that has pages.
    pub(crate) footer: Option<gtk::Widget>,
    /// A button before Cancel that the step draws itself, such as a carousel's Back.
    pub(crate) start: Option<gtk::Widget>,
}

/// The host. Its owner keeps it for as long as it is on screen.
pub(crate) struct FlowHost {
    view: adw::NavigationView,
    /// Set by a press that moves the flow forward, and spent by the next step drawn.
    forward: Cell<bool>,
    /// Set while the host pops a page itself, which is not the person's Back.
    popping: Cell<bool>,
    back: RefCell<Option<Pressed>>,
    close: RefCell<Option<Pressed>>,
    /// Each page's frame as drawn last, so the widgets a step keeps between draws (a carousel, its
    /// dots) are taken out of the old frame before the new one holds them.
    frames: RefCell<HashMap<&'static str, Frame>>,
}

struct Frame {
    toolbar: adw::ToolbarView,
    header: adw::HeaderBar,
    start: Option<gtk::Widget>,
    footer: Option<gtk::Widget>,
}

impl Frame {
    fn release(&self) {
        if let Some(start) = &self.start {
            self.header.remove(start);
        }
        if let Some(footer) = &self.footer {
            self.toolbar.remove(footer);
        }
        self.header.set_title_widget(None::<&gtk::Widget>);
        self.toolbar.set_content(None::<&gtk::Widget>);
    }
}

impl FlowHost {
    /// A host that tells `back` when the person goes back and `close` when they cancel.
    pub(crate) fn new(back: Pressed, close: Pressed) -> Rc<Self> {
        let view = adw::NavigationView::new();
        view.set_hexpand(true);
        view.set_vexpand(true);
        let host = Rc::new(Self {
            view,
            forward: Cell::new(false),
            popping: Cell::new(false),
            back: RefCell::new(Some(back)),
            close: RefCell::new(Some(close)),
            frames: RefCell::default(),
        });
        let weak = Rc::downgrade(&host);
        host.view.connect_popped(move |_, _| {
            let Some(host) = weak.upgrade() else {
                return;
            };
            if !host.popping.get() {
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

    /// Draws `step`: in place when it is the step on screen, popped back to when it is below,
    /// pushed after a press that moved forward, and otherwise in place of the page on top.
    pub(crate) fn show(self: &Rc<Self>, step: FlowStep) {
        let forward = self.forward.replace(false);
        let can_pop = step.back.is_some_and(|back| back.enabled);
        let offers_back = step.back.is_some();
        let tag = step.tag;
        let page = if let Some(page) = self.view.find_page(tag).filter(|page| self.holds(page)) {
            if self.view.visible_page().as_ref() != Some(&page) {
                self.quietly(|view| {
                    view.pop_to_page(&page);
                });
            }
            page
        } else {
            let page = adw::NavigationPage::with_tag(
                &gtk::Box::new(gtk::Orientation::Vertical, 0),
                &step.title,
                tag,
            );
            let mut stack = self.stack();
            if forward && !stack.is_empty() {
                stack.push(page.clone());
            } else {
                stack.pop();
                stack.push(page.clone());
            }
            self.quietly(|view| view.replace(&stack));
            page
        };
        page.set_title(&step.title);
        if let Some(old) = self.frames.borrow_mut().remove(tag) {
            old.release();
        }
        let frame = self.frame(step);
        page.set_child(Some(&frame.toolbar));
        self.frames.borrow_mut().insert(tag, frame);
        page.set_can_pop(can_pop);
        // A first step that offers Back still needs a page below it for the header bar to draw
        // the button; going back to it is the person's Back, like any other.
        let stack = self.stack();
        if offers_back && stack.len() == 1 {
            let below = adw::NavigationPage::with_tag(&adw::Bin::new(), "", BELOW);
            self.quietly(|view| view.replace(&[below, page.clone()]));
        } else if !offers_back {
            let at = stack.iter().position(|candidate| candidate == &page);
            if let Some(at) = at.filter(|at| *at > 0)
                && stack[at - 1].tag().as_deref() == Some(BELOW)
            {
                self.quietly(|view| view.replace(&stack[at..]));
            }
        }
    }

    /// Takes the host off screen: nothing it does afterwards reaches its owner.
    pub(crate) fn dismiss(&self) {
        self.back.replace(None);
        self.close.replace(None);
    }

    /// Whether `page` is on the stack, rather than only known to the view.
    fn holds(&self, page: &adw::NavigationPage) -> bool {
        self.stack().contains(page)
    }

    fn stack(&self) -> Vec<adw::NavigationPage> {
        let model = self.view.navigation_stack();
        (0..model.n_items())
            .filter_map(|position| model.item(position).and_downcast::<adw::NavigationPage>())
            .collect()
    }

    fn quietly(&self, change: impl FnOnce(&adw::NavigationView)) {
        self.popping.set(true);
        change(&self.view);
        self.popping.set(false);
    }

    /// The step's toolbar: Cancel at the start, the primary at the end, the content between and
    /// what the step draws under it.
    fn frame(self: &Rc<Self>, step: FlowStep) -> Frame {
        let toolbar = adw::ToolbarView::new();
        let header = adw::HeaderBar::new();
        // The Settings window's own title bar closes the window; Cancel closes the flow.
        header.set_show_end_title_buttons(false);
        header.set_show_start_title_buttons(false);
        let title = step
            .title_widget
            .unwrap_or_else(|| adw::WindowTitle::new(&step.title, "").upcast::<gtk::Widget>());
        header.set_title_widget(Some(&title));
        if let Some(start) = &step.start {
            header.pack_start(start);
        }
        let weak = Rc::downgrade(self);
        let cancel = controls::button(l10n::action_cancel(), step.close, false, move || {
            let close = weak.upgrade().and_then(|host| host.close.borrow().clone());
            if let Some(close) = close {
                close();
            }
        });
        wraps(&cancel);
        header.pack_start(&cancel);
        if let Some(primary) = step.primary {
            let weak = Rc::downgrade(self);
            let (advances, pressed) = (primary.advances, primary.pressed);
            let button = controls::button(&primary.label, primary.control, false, move || {
                if let Some(host) = weak.upgrade() {
                    host.forward.set(advances);
                }
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
            start: step.start,
            footer: step.footer,
        }
    }
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
