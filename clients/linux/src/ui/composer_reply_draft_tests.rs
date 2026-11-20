//! Draft a reply as the core's record draws it (`docs/ai.md` rules 17 to 24): what each state says
//! among the notes, the control's and Draft's states, the input held with its remedy, and the two
//! questions. Called from the crate's single `gtk::init` test through [`all`].

use std::{cell::RefCell, rc::Rc};

use adw::prelude::*;
use mailcal_bindings::{
    AiFailure, AiRemedy, AiRemedyControl, ComposerReplyDraft, ControlState, ReplyDraftHold,
    ReplyDraftItem, ReplyDraftState, ReplyDraftSuggestion, ReplyDraftTaskKind,
};

use super::{CANCEL, KEEP, REPLACE, SEND, replace_question, send_question};
use crate::{
    l10n,
    ui::{
        composer_reply_draft_input::{self as input, Input},
        composer_reply_draft_notes::{self as notes},
        controls::{self, tests::spins},
        icons,
        mailbox::tests::rendered_labels,
    },
};

const FREE: ControlState = ControlState {
    enabled: true,
    running: false,
};
const HELD: ControlState = ControlState {
    enabled: false,
    running: false,
};
const RUNNING: ControlState = ControlState {
    enabled: false,
    running: true,
};

type Asked = Rc<RefCell<Vec<String>>>;

pub(crate) fn all() {
    each_state_says_what_it_is_among_the_notes();
    the_card_ticks_what_the_core_ticks_and_marks_a_gap();
    the_control_runs_while_a_draft_is_made();
    the_input_fills_from_a_suggestion_and_drafts_on_return();
    the_input_held_says_why_with_its_remedy();
    the_questions_offer_their_two_answers();
}

fn record(state: ReplyDraftState) -> ComposerReplyDraft {
    ComposerReplyDraft {
        state,
        control: FREE,
        draft: FREE,
        stop: FREE,
        held_by: None,
        suggestions: vec![ReplyDraftSuggestion::Yes, ReplyDraftSuggestion::Holding],
        instruction: String::new(),
        ask_before_replacing: false,
        replace_arrived: false,
        editor_script: None,
        send_question: None,
    }
}

fn note_actions(asked: &Asked) -> notes::Actions {
    let (stop, tick, remedy) = (asked.clone(), asked.clone(), asked.clone());
    notes::Actions {
        stop: Rc::new(move || stop.borrow_mut().push("stop".to_owned())),
        tick: Rc::new(move |index, done| tick.borrow_mut().push(format!("tick {index} {done}"))),
        remedy: Rc::new(move |chosen| remedy.borrow_mut().push(format!("remedy {chosen:?}"))),
    }
}

fn descendants<T: IsA<gtk::Widget>>(root: &gtk::Widget) -> Vec<T> {
    let mut found = Vec::new();
    let mut child = root.first_child();
    while let Some(widget) = child {
        if let Some(wanted) = widget.downcast_ref::<T>() {
            found.push(wanted.clone());
        }
        found.extend(descendants::<T>(&widget));
        child = widget.next_sibling();
    }
    found
}

fn button(root: &gtk::Widget, label: &str) -> Option<gtk::Button> {
    descendants::<gtk::Button>(root).into_iter().find(|button| {
        rendered_labels(button.upcast_ref())
            .iter()
            .any(|text| text == label)
    })
}

fn drawn(record: &ComposerReplyDraft, asked: &Asked) -> gtk::Box {
    let box_ = gtk::Box::new(gtk::Orientation::Vertical, 0);
    notes::draw(&box_, record, &note_actions(asked));
    box_
}

fn each_state_says_what_it_is_among_the_notes() {
    let asked = Asked::default();
    assert!(
        !drawn(&record(ReplyDraftState::Ready), &asked).is_visible(),
        "ready: nothing among the notes"
    );

    let drafting = drawn(&record(ReplyDraftState::Drafting), &asked);
    let root = drafting.upcast_ref::<gtk::Widget>();
    assert!(
        rendered_labels(root)
            .iter()
            .any(|text| text == l10n::composer_drafting())
    );
    assert_eq!(
        descendants::<adw::Spinner>(root).len(),
        0,
        "the action row's control is the one spinner while a draft is made"
    );
    button(root, l10n::composer_draft_stop())
        .expect("Stop")
        .emit_clicked();

    let remedy = AiRemedyControl {
        remedy: AiRemedy::TryAgain,
        control: FREE,
    };
    let failed = drawn(
        &record(ReplyDraftState::Failed {
            failure: AiFailure::Unreachable,
            remedy: Some(remedy),
        }),
        &asked,
    );
    let root = failed.upcast_ref::<gtk::Widget>();
    assert!(
        rendered_labels(root)
            .iter()
            .any(|text| text == l10n::ai_failure_unreachable())
    );
    button(root, l10n::ai_remedy_try_again())
        .expect("its remedy")
        .emit_clicked();

    // A reason the person can act on is said in the input, so the notes say nothing of it; the
    // one they cannot act on is a line here.
    let held = ComposerReplyDraft {
        held_by: Some(ReplyDraftHold {
            failure: AiFailure::NoStyle,
            remedy: None,
        }),
        ..record(ReplyDraftState::Unavailable {
            failure: AiFailure::NoStyle,
            remedy: None,
        })
    };
    assert!(!drawn(&held, &asked).is_visible());
    let gone = ComposerReplyDraft {
        control: HELD,
        ..record(ReplyDraftState::Unavailable {
            failure: AiFailure::MessageGone,
            remedy: None,
        })
    };
    assert!(
        rendered_labels(drawn(&gone, &asked).upcast_ref())
            .iter()
            .any(|text| text == l10n::ai_failure_message_gone())
    );
    assert_eq!(*asked.borrow(), ["stop", "remedy TryAgain"]);
}

fn the_card_ticks_what_the_core_ticks_and_marks_a_gap() {
    let item = |index, kind, text: &str, done, tick| ReplyDraftItem {
        index,
        kind,
        text: text.to_owned(),
        done,
        tick,
    };
    let asked = Asked::default();
    let card = drawn(
        &record(ReplyDraftState::Drafted {
            summary: "Bob asks whether Saturday suits you.".to_owned(),
            checklist: vec![
                item(0, ReplyDraftTaskKind::FillIn, "[date]", true, None),
                item(1, ReplyDraftTaskKind::FillIn, "[time]", false, None),
                item(
                    2,
                    ReplyDraftTaskKind::Attach,
                    "Attach the agenda",
                    false,
                    Some(FREE),
                ),
                item(
                    3,
                    ReplyDraftTaskKind::Do,
                    "Book the room",
                    false,
                    Some(FREE),
                ),
            ],
        }),
        &asked,
    );
    let root = card.upcast_ref::<gtk::Widget>();
    let shown = rendered_labels(root);
    assert!(
        shown
            .iter()
            .any(|text| text == l10n::composer_draft_summary_heading())
    );
    assert!(
        shown
            .iter()
            .any(|text| text == l10n::composer_checklist_heading())
    );
    let checks = descendants::<gtk::CheckButton>(root);
    assert_eq!(
        checks.len(),
        2,
        "a gap follows the reply, so it is no checkbox"
    );
    let gaps = descendants::<gtk::Box>(root)
        .into_iter()
        .filter(|line| line.widget_name() == notes::GAP_MARK)
        .collect::<Vec<_>>();
    assert_eq!(gaps.len(), 2);
    let marks = |line: &gtk::Box| {
        descendants::<gtk::Image>(line.upcast_ref())
            .iter()
            .filter_map(|image| image.icon_name().map(|name| name.to_string()))
            .collect::<Vec<_>>()
    };
    assert_eq!(
        marks(&gaps[0]),
        [icons::GAP_FILLED],
        "a gap filled in is done"
    );
    assert_eq!(
        marks(&gaps[1]),
        [icons::GAP_OPEN],
        "a gap still in the reply is open"
    );
    assert!(
        gaps.iter()
            .all(|line| line.accessible_role() == gtk::AccessibleRole::Checkbox)
    );
    assert!(shown.contains(&l10n::composer_task_fill_in("[date]")));
    assert!(!checks[1].is_active() && checks[1].is_sensitive());
    checks[1].set_active(true);
    assert_eq!(
        *asked.borrow(),
        ["tick 3 true"],
        "drawing asked for nothing, a tick asks once"
    );
}

fn the_control_runs_while_a_draft_is_made() {
    let popover = gtk::Popover::new();
    let running = controls::menu_button(l10n::composer_draft_reply(), RUNNING, &popover);
    assert!(!running.is_sensitive());
    assert_eq!(descendants::<adw::Spinner>(running.upcast_ref()).len(), 1);
    running.set_popover(None::<&gtk::Popover>);
    let free = controls::menu_button(l10n::composer_draft_reply(), FREE, &popover);
    assert!(free.is_sensitive());
    assert_eq!(descendants::<adw::Spinner>(free.upcast_ref()).len(), 0);
}

fn input_actions(asked: &Asked) -> input::Actions {
    let (draft, remedy) = (asked.clone(), asked.clone());
    input::Actions {
        draft: Rc::new(move |line| draft.borrow_mut().push(format!("draft {line}"))),
        remedy: Rc::new(move |chosen| remedy.borrow_mut().push(format!("remedy {chosen:?}"))),
    }
}

fn the_input_fills_from_a_suggestion_and_drafts_on_return() {
    let asked = Asked::default();
    let opened = Input::new(
        &ComposerReplyDraft {
            instruction: "Say yes".to_owned(),
            ..record(ReplyDraftState::Ready)
        },
        input_actions(&asked),
    );
    let root = opened.widget();
    let entry = descendants::<gtk::Entry>(&root).pop().expect("the line");
    assert_eq!(
        entry.text(),
        "Say yes",
        "it opens holding the last instruction"
    );
    button(&root, l10n::composer_draft_chip_later())
        .expect("a suggestion")
        .emit_clicked();
    assert_eq!(entry.text(), l10n::composer_draft_chip_later());
    entry.emit_activate();
    let draft = button(&root, l10n::composer_draft_create()).expect("Draft");
    assert!(draft.is_sensitive());
    opened.update(&ComposerReplyDraft {
        draft: RUNNING,
        ..record(ReplyDraftState::Drafting)
    });
    let draft = button(&root, l10n::composer_draft_create()).expect("Draft");
    assert!(spins(&draft) && !draft.is_sensitive());
    assert_eq!(
        *asked.borrow(),
        [format!("draft {}", l10n::composer_draft_chip_later())]
    );
}

fn the_input_held_says_why_with_its_remedy() {
    let asked = Asked::default();
    let opened = Input::new(
        &ComposerReplyDraft {
            held_by: Some(ReplyDraftHold {
                failure: AiFailure::NoStyleForAccount,
                remedy: Some(AiRemedyControl {
                    remedy: AiRemedy::ChooseStyle,
                    control: FREE,
                }),
            }),
            ..record(ReplyDraftState::Unavailable {
                failure: AiFailure::NoStyleForAccount,
                remedy: None,
            })
        },
        input_actions(&asked),
    );
    let root = opened.widget();
    assert!(
        descendants::<gtk::Entry>(&root).is_empty(),
        "the reason in the line's place"
    );
    assert!(
        rendered_labels(&root)
            .iter()
            .any(|text| text == l10n::ai_failure_no_style_for_account())
    );
    button(&root, l10n::ai_remedy_choose_style())
        .expect("its remedy")
        .emit_clicked();
    assert_eq!(*asked.borrow(), ["remedy ChooseStyle"]);
}

fn the_questions_offer_their_two_answers() {
    let asked = send_question(2);
    assert_eq!(
        asked.heading().as_deref(),
        Some(l10n::composer_send_open_title())
    );
    assert_eq!(asked.body(), l10n::composer_send_open_message(2));
    assert!(asked.has_response(SEND) && asked.has_response(KEEP));
    assert_eq!(asked.close_response(), KEEP, "closing it keeps editing");

    let replace = replace_question();
    assert!(replace.has_response(REPLACE) && replace.has_response(CANCEL));
    assert_eq!(
        replace.close_response(),
        CANCEL,
        "closing it keeps the person's text"
    );
}
