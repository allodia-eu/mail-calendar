//! What the flow host promises: its stack is the record's trail with the step on top, a step draws
//! the controls it is given and nothing else, and the start of the header holds Back where the
//! step offers it and the one way out where it does not. Called from the crate's single
//! `gtk::init` test.

use std::{cell::RefCell, rc::Rc};

use adw::prelude::*;
use mailcal_bindings::ControlState;

use super::{FlowAction, FlowHost, FlowLeave, FlowStep, FlowTrailStep};
use crate::ui::{controls::tests::spins, mailbox::tests::rendered_labels};

const FREE: ControlState = ControlState {
    enabled: true,
    running: false,
};

fn step(tag: &'static str, back: Option<ControlState>, primary: Option<FlowAction>) -> FlowStep {
    FlowStep {
        tag,
        title: tag.to_owned(),
        title_widget: None,
        content: gtk::Label::new(Some(tag)).upcast(),
        back,
        leave: FlowLeave {
            label: "Cancel".to_owned(),
            control: FREE,
        },
        own_back: None,
        primary,
        footer: None,
    }
}

fn trail(tags: &[&'static str]) -> Vec<FlowTrailStep> {
    tags.iter()
        .map(|tag| FlowTrailStep {
            tag,
            title: (*tag).to_owned(),
        })
        .collect()
}

fn next(asked: &Rc<RefCell<Vec<&'static str>>>, control: ControlState) -> FlowAction {
    let asked = Rc::clone(asked);
    FlowAction {
        label: "Next".to_owned(),
        control,
        suggested: true,
        pressed: Rc::new(move || asked.borrow_mut().push("next")),
    }
}

fn tags(host: &FlowHost) -> Vec<String> {
    host.stack()
        .iter()
        .map(|page| page.tag().map(|tag| tag.to_string()).unwrap_or_default())
        .collect()
}

fn buttons(root: &gtk::Widget) -> Vec<gtk::Button> {
    let mut found = Vec::new();
    let mut child = root.first_child();
    while let Some(widget) = child {
        if let Some(button) = widget.downcast_ref::<gtk::Button>() {
            found.push(button.clone());
        }
        found.extend(buttons(&widget));
        child = widget.next_sibling();
    }
    found
}

fn labelled(root: &gtk::Widget, label: &str) -> Option<gtk::Button> {
    buttons(root).into_iter().find(|button| {
        rendered_labels(button.upcast_ref())
            .iter()
            .any(|text| text == label)
    })
}

pub(crate) fn the_stack_is_the_trail_with_the_step_on_top() {
    let asked = Rc::new(RefCell::new(Vec::new()));
    let backs = Rc::new(RefCell::new(0));
    let counted = Rc::clone(&backs);
    let host = FlowHost::new(Rc::new(move || *counted.borrow_mut() += 1), Rc::new(|| {}));
    host.show(&[], step("account", None, Some(next(&asked, FREE))));
    assert_eq!(tags(&host), ["account"]);
    let page = host.view.visible_page().expect("a page");
    labelled(page.upcast_ref(), "Next")
        .expect("Next is drawn")
        .emit_clicked();
    assert_eq!(*asked.borrow(), ["next"]);

    host.show(&trail(&["account"]), step("range", Some(FREE), None));
    assert_eq!(tags(&host), ["account", "range"], "one further is a push");
    assert!(host.view.visible_page().is_some_and(|page| page.can_pop()));

    host.show(
        &trail(&["account", "range"]),
        step("failed", Some(FREE), None),
    );
    host.show(&trail(&["account"]), step("review", Some(FREE), None));
    assert_eq!(
        tags(&host),
        ["account", "review"],
        "anything else is the record's stack"
    );
    host.show(&[], step("account", None, None));
    assert_eq!(tags(&host), ["account"], "a step below is popped back to");
    host.show(&trail(&["range", "review"]), step("learning", None, None));
    assert_eq!(tags(&host), ["range", "review", "learning"]);
    assert_eq!(
        *backs.borrow(),
        0,
        "the host's own moves are not the person's Back"
    );
}

/// Back is the header's back button where the step offers it, and the way out stands at the
/// start where it does not: never both.
pub(crate) fn the_start_holds_back_or_the_way_out() {
    let host = FlowHost::new(Rc::new(|| {}), Rc::new(|| {}));
    host.show(&trail(&["review"]), step("failed", Some(FREE), None));
    let page = host.view.visible_page().expect("a page");
    assert!(page.can_pop());
    assert!(labelled(page.upcast_ref(), "Cancel").is_none());
    let bar = host
        .frames
        .borrow()
        .get("failed")
        .map(|frame| frame.header.clone());
    let bar = bar.expect("the step's bar");
    assert!(
        bar.shows_end_title_buttons() && !bar.shows_start_title_buttons(),
        "the bar stands in for the window's own: its close is the way out beside Back"
    );

    host.show(&trail(&["range", "review"]), step("learning", None, None));
    let page = host.view.visible_page().expect("a page");
    assert!(!page.can_pop(), "no Back where the step offers none");
    assert!(labelled(page.upcast_ref(), "Cancel").is_some());
}

pub(crate) fn a_step_draws_only_the_controls_it_is_given() {
    let asked = Rc::new(RefCell::new(Vec::new()));
    let host = FlowHost::new(Rc::new(|| {}), Rc::new(|| {}));
    let running = ControlState {
        enabled: false,
        running: true,
    };
    host.show(&[], step("learning", None, Some(next(&asked, running))));
    let page = host.view.visible_page().expect("a page");
    let primary = labelled(page.upcast_ref(), "Next").expect("the primary");
    assert!(spins(&primary) && !primary.is_sensitive(), "its work runs");

    host.show(&[], step("learning", None, None));
    let page = host.view.visible_page().expect("a page");
    assert!(
        labelled(page.upcast_ref(), "Next").is_none(),
        "none given, none drawn"
    );
}
