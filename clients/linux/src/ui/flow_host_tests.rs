//! What the flow host promises: a step is drawn from the controls it is given and nothing else,
//! a step forward is pushed, a step already below is popped back to, and the header bar draws Back
//! exactly while the step offers it. Called from the crate's single `gtk::init` test.

use std::{cell::RefCell, rc::Rc};

use adw::prelude::*;
use mailcal_bindings::ControlState;

use super::{FlowAction, FlowHost, FlowStep};
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
        primary,
        close: FREE,
        footer: None,
        start: None,
    }
}

fn next(asked: &Rc<RefCell<Vec<&'static str>>>, control: ControlState) -> FlowAction {
    let asked = Rc::clone(asked);
    FlowAction {
        label: "Next".to_owned(),
        control,
        suggested: true,
        advances: true,
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

pub(crate) fn a_step_forward_is_pushed_and_one_below_is_popped_back_to() {
    let asked = Rc::new(RefCell::new(Vec::new()));
    let backs = Rc::new(RefCell::new(0));
    let counted = Rc::clone(&backs);
    let host = FlowHost::new(Rc::new(move || *counted.borrow_mut() += 1), Rc::new(|| {}));
    host.show(step("account", None, Some(next(&asked, FREE))));
    assert_eq!(tags(&host), ["account"]);

    let page = host.view.visible_page().expect("a page");
    let pressed = labelled(page.upcast_ref(), "Next").expect("Next is drawn");
    pressed.emit_clicked();
    assert_eq!(*asked.borrow(), ["next"]);
    host.show(step("range", Some(FREE), Some(next(&asked, FREE))));
    assert_eq!(
        tags(&host),
        ["account", "range"],
        "a step forward is pushed"
    );
    assert!(host.view.visible_page().is_some_and(|page| page.can_pop()));

    // The core moved on by itself: the step replaces the one on top.
    host.show(step("review", Some(FREE), None));
    assert_eq!(tags(&host), ["account", "review"]);
    host.show(step("account", None, Some(next(&asked, FREE))));
    assert_eq!(tags(&host), ["account"], "a step below is popped back to");
    assert_eq!(
        *backs.borrow(),
        0,
        "the host's own pops are not the person's Back"
    );
}

pub(crate) fn a_first_step_that_offers_back_still_draws_it() {
    let host = FlowHost::new(Rc::new(|| {}), Rc::new(|| {}));
    host.show(step("failed", Some(FREE), None));
    assert_eq!(tags(&host), [super::BELOW, "failed"]);
    assert!(host.view.visible_page().is_some_and(|page| page.can_pop()));
    host.show(step("learning", None, None));
    assert_eq!(
        tags(&host),
        ["learning"],
        "nothing to go back to: no back button"
    );
}

pub(crate) fn a_step_draws_only_the_controls_it_is_given() {
    let asked = Rc::new(RefCell::new(Vec::new()));
    let host = FlowHost::new(Rc::new(|| {}), Rc::new(|| {}));
    let running = ControlState {
        enabled: false,
        running: true,
    };
    host.show(step("learning", None, Some(next(&asked, running))));
    let page = host.view.visible_page().expect("a page");
    let root = page.upcast_ref::<gtk::Widget>();
    let primary = labelled(root, "Next").expect("the primary");
    assert!(spins(&primary) && !primary.is_sensitive(), "its work runs");
    assert!(!page.can_pop(), "no Back where the step offers none");
    assert!(labelled(root, crate::l10n::action_cancel()).is_some());

    host.show(step("learning", None, None));
    let page = host.view.visible_page().expect("a page");
    assert!(
        labelled(page.upcast_ref(), "Next").is_none(),
        "none given, none drawn"
    );
}
