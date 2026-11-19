//! Settings → Writing style as the core's records draw it (`docs/ai.md` rules 8 to 16): the page,
//! each stage of learning in the flow host, and the reveal acting on an outcome once. Called from
//! the crate's single `gtk::init` test through [`all`].

use std::{cell::RefCell, rc::Rc};

use adw::prelude::*;
use mailcal_bindings::{
    AiFailure, AiRemedy, AiRemedyControl, ControlState, StyleLearnRange, StyleLearningAccount,
    StyleLearningDestination, StyleLearningIntent, StyleLearningReport, StyleLearningReview,
    StyleLearningStage, StyleLearningTrailStage, StyleSentReach, WritingStyleChangeKind,
    WritingStyleEntry, WritingStyleFacts, WritingStyleLearnBlock, WritingStyleLearning,
    WritingStyleOpens, WritingStyleOutcome, WritingStylePage, WritingStyleRefusal,
    WritingStyleResult, WritingStyleReveal, WritingStyleRevealPage, WritingStyleRun,
    WritingStyleSlot,
};

use crate::{
    l10n,
    ui::{
        flow_host::tests as flow_host,
        mailbox::tests::rendered_labels,
        settings::{
            ai_endpoint::tests as endpoint,
            writing_style_learning::{Learning, Wiring},
            writing_style_page::{Actions, draw},
            writing_style_reveal::Reveal,
        },
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

type Asked = Rc<RefCell<Vec<String>>>;

pub(crate) fn all() {
    the_page_draws_the_library_learn_and_the_slots();
    learn_says_why_it_is_held_with_its_one_remedy();
    a_run_is_drawn_in_learns_place_with_stop();
    a_run_that_failed_says_so_and_learn_reopens_it();
    each_stage_draws_exactly_the_controls_its_record_gives();
    a_remedy_the_flow_moves_to_is_sent_and_any_other_is_opened();
    the_reveal_acts_on_an_outcome_once();
    endpoint::the_form_is_absent_while_the_core_offers_none();
    endpoint::save_sends_the_fields_as_typed_and_no_key_for_an_empty_field();
    endpoint::a_gap_is_said_under_its_field_and_holds_save();
    endpoint::a_running_save_holds_the_form_and_a_refused_key_is_said();
    flow_host::the_stack_is_the_trail_with_the_step_on_top();
    flow_host::the_start_holds_back_or_the_way_out();
    flow_host::a_step_draws_only_the_controls_it_is_given();
}

fn facts() -> WritingStyleFacts {
    WritingStyleFacts {
        messages: 8,
        learned_from: "alice@test.local".to_owned(),
        since: Some(1_760_000_000),
        learned_at: Some(1_763_000_000),
        languages: vec!["en".to_owned(), "nl".to_owned()],
    }
}

fn page() -> WritingStylePage {
    WritingStylePage {
        styles: vec![WritingStyleEntry {
            id: "style-1".to_owned(),
            name: "My style".to_owned(),
            facts: facts(),
            opens: WritingStyleOpens::Detail,
            open: FREE,
        }],
        accounts: vec![WritingStyleSlot {
            account_id: "account-1".to_owned(),
            email: "alice@test.local".to_owned(),
            selected: Some("style-1".to_owned()),
            picker: FREE,
        }],
        learn: FREE,
        learn_blocked: None,
        run: None,
        outcome: None,
    }
}

fn actions(asked: &Asked) -> Actions {
    let (open, assign, learn, stop, remedy) = (
        asked.clone(),
        asked.clone(),
        asked.clone(),
        asked.clone(),
        asked.clone(),
    );
    Actions {
        open: Rc::new(move |id, opens| open.borrow_mut().push(format!("open {id} {opens:?}"))),
        assign: Rc::new(move |account, style| {
            assign
                .borrow_mut()
                .push(format!("assign {account} {style:?}"));
        }),
        learn: Rc::new(move || learn.borrow_mut().push("learn".to_owned())),
        stop: Rc::new(move || stop.borrow_mut().push("stop".to_owned())),
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

fn drawn(page: &WritingStylePage, asked: &Asked) -> gtk::Box {
    let body = gtk::Box::new(gtk::Orientation::Vertical, 0);
    draw(&body, page, None, &actions(asked));
    body
}

fn the_page_draws_the_library_learn_and_the_slots() {
    let asked = Asked::default();
    let body = drawn(&page(), &asked);
    let root = body.upcast_ref::<gtk::Widget>();
    assert!(asked.borrow().is_empty(), "drawing asks for nothing");
    assert!(rendered_labels(root).iter().any(|text| text == "My style"));
    let learn = button(root, l10n::writing_style_learn()).expect("Learn");
    assert!(learn.is_sensitive());
    learn.emit_clicked();
    let slots = descendants::<adw::ComboRow>(root);
    assert_eq!(slots.len(), 1);
    assert_eq!(
        slots[0].selected(),
        1,
        "None, then the library: on My style"
    );
    slots[0].set_selected(0);
    assert_eq!(*asked.borrow(), ["learn", "assign account-1 None"]);
}

fn learn_says_why_it_is_held_with_its_one_remedy() {
    let asked = Asked::default();
    let body = drawn(
        &WritingStylePage {
            learn: HELD,
            learn_blocked: Some(WritingStyleLearnBlock::CannotSend {
                failure: AiFailure::NowhereToSend,
                remedy: Some(AiRemedyControl {
                    remedy: AiRemedy::OpenAiEndpoint,
                    control: FREE,
                }),
            }),
            ..page()
        },
        &asked,
    );
    let root = body.upcast_ref::<gtk::Widget>();
    assert!(
        rendered_labels(root)
            .iter()
            .any(|text| text == l10n::ai_failure_nowhere_to_send())
    );
    assert!(
        !button(root, l10n::writing_style_learn())
            .expect("Learn")
            .is_sensitive()
    );
    button(root, l10n::ai_remedy_open_endpoint())
        .expect("its remedy")
        .emit_clicked();
    assert_eq!(*asked.borrow(), ["remedy OpenAiEndpoint"]);
}

fn a_run_is_drawn_in_learns_place_with_stop() {
    for (total, said) in [
        (4, l10n::learn_progress("1", "4")),
        (0, l10n::learn_progress_starting().to_owned()),
    ] {
        let asked = Asked::default();
        let body = drawn(
            &WritingStylePage {
                learn: HELD,
                run: Some(WritingStyleRun::Learning {
                    done: 1,
                    total,
                    stop: FREE,
                }),
                ..page()
            },
            &asked,
        );
        let root = body.upcast_ref::<gtk::Widget>();
        assert!(
            button(root, l10n::writing_style_learn()).is_none(),
            "in Learn's place"
        );
        assert!(rendered_labels(root).contains(&said));
        button(root, l10n::learn_stop())
            .expect("Stop")
            .emit_clicked();
        assert_eq!(*asked.borrow(), ["stop"]);
    }
}

fn a_run_that_failed_says_so_and_learn_reopens_it() {
    let body = drawn(
        &WritingStylePage {
            run: Some(WritingStyleRun::Failed {
                failure: AiFailure::Unreachable,
                remedy: Some(AiRemedyControl {
                    remedy: AiRemedy::TryAgain,
                    control: FREE,
                }),
            }),
            ..page()
        },
        &Asked::default(),
    );
    let root = body.upcast_ref::<gtk::Widget>();
    let shown = rendered_labels(root);
    assert!(
        shown
            .iter()
            .any(|text| text == l10n::writing_style_run_failed())
    );
    assert!(
        shown
            .iter()
            .any(|text| text == l10n::ai_failure_unreachable())
    );
    assert!(button(root, l10n::writing_style_learn()).is_some_and(|learn| learn.is_sensitive()));
}

fn flow(asked: &Asked) -> Rc<Learning> {
    let (send, style, open) = (asked.clone(), asked.clone(), asked.clone());
    Learning::new(Wiring {
        send: Rc::new(move |intent: StyleLearningIntent| {
            send.borrow_mut().push(said(&intent));
        }),
        style: Rc::new(move |_| style.borrow_mut().push("style".to_owned())),
        reveal_of: Rc::new(|_| None),
        open_remedy: Rc::new(move |remedy| open.borrow_mut().push(format!("open {remedy:?}"))),
    })
}

fn record(stage: StyleLearningStage) -> WritingStyleLearning {
    WritingStyleLearning {
        stage,
        back: None,
        next: None,
        learn: None,
        stop: None,
        close: FREE,
        trail: Vec::new(),
    }
}

fn on_screen(flow: &Learning) -> gtk::Widget {
    flow.host.widget().visible_page().expect("a page").upcast()
}

fn each_stage_draws_exactly_the_controls_its_record_gives() {
    let asked = Asked::default();
    let flow = flow(&asked);
    flow.draw(
        WritingStyleLearning {
            next: Some(HELD),
            ..record(StyleLearningStage::ChooseAccount {
                accounts: vec![StyleLearningAccount {
                    account_id: "account-1".to_owned(),
                    email: "alice@test.local".to_owned(),
                }],
                selected: None,
            })
        },
        None,
    );
    let page = on_screen(&flow);
    assert!(
        !button(&page, l10n::wizard_next())
            .expect("Next")
            .is_sensitive()
    );
    assert!(
        !flow.host.widget().visible_page().expect("a page").can_pop(),
        "no Back"
    );
    descendants::<gtk::CheckButton>(&page)[0].set_active(true);

    let report = StyleLearningReport {
        found: 8,
        usable: 6,
        languages: Vec::new(),
        other_languages: 0,
        oldest: None,
        newest: None,
    };
    flow.draw(
        WritingStyleLearning {
            back: Some(FREE),
            learn: Some(FREE),
            ..record(StyleLearningStage::Review {
                email: "alice@test.local".to_owned(),
                range: StyleLearnRange::Everything,
                reach: StyleSentReach::Everything,
                review: StyleLearningReview::Ready { report },
                sends_to: Some(StyleLearningDestination::OwnEndpoint {
                    host: "localhost".to_owned(),
                }),
                cannot_send: None,
                keyword_kept: false,
            })
        },
        None,
    );
    let page = on_screen(&flow);
    let shown = rendered_labels(&page);
    assert!(shown.contains(&l10n::learn_consent_own("localhost")));
    assert!(
        shown
            .iter()
            .any(|text| text == l10n::learn_keyword_not_kept())
    );
    assert!(
        button(&page, l10n::wizard_next()).is_none(),
        "none given, none drawn"
    );
    button(&page, l10n::learn_consent_confirm())
        .expect("Learn")
        .emit_clicked();

    flow.draw(
        WritingStyleLearning {
            stop: Some(FREE),
            ..record(StyleLearningStage::Learning { done: 0, total: 0 })
        },
        None,
    );
    let page = on_screen(&flow);
    assert!(button(&page, l10n::learn_consent_confirm()).is_none());
    assert!(!flow.host.widget().visible_page().expect("a page").can_pop());
    button(&page, l10n::learn_stop())
        .expect("Stop")
        .emit_clicked();
    button(&page, l10n::action_cancel())
        .expect("Cancel")
        .emit_clicked();
    let learn = format!(
        "Learn {} {}",
        l10n::writing_style_default_name(),
        l10n::active_locale()
    );
    assert_eq!(
        *asked.borrow(),
        [
            "SelectAccount account-1".to_owned(),
            learn,
            "Stop".to_owned(),
            "Close".to_owned()
        ]
    );
}

fn a_remedy_the_flow_moves_to_is_sent_and_any_other_is_opened() {
    for (remedy, expected) in [
        (AiRemedy::ChooseRange, vec!["Remedy"]),
        (
            AiRemedy::OpenAiEndpoint,
            vec!["Close", "open OpenAiEndpoint"],
        ),
    ] {
        let asked = Asked::default();
        let flow = flow(&asked);
        flow.draw(
            WritingStyleLearning {
                back: Some(FREE),
                trail: vec![StyleLearningTrailStage::Review],
                ..record(StyleLearningStage::Failed {
                    failure: AiFailure::NothingToLearn,
                    remedy: Some(AiRemedyControl {
                        remedy,
                        control: FREE,
                    }),
                })
            },
            None,
        );
        let page = on_screen(&flow);
        assert!(
            rendered_labels(&page)
                .iter()
                .any(|text| text == l10n::learn_failed_title())
        );
        button(
            &page,
            crate::ui::settings::writing_style_words::remedy(remedy),
        )
        .expect("its remedy")
        .emit_clicked();
        assert_eq!(*asked.borrow(), expected);
    }
}

fn the_reveal_acts_on_an_outcome_once() {
    let reveal = Reveal::new(
        WritingStyleReveal {
            id: "style-1".to_owned(),
            facts: facts(),
            pages: vec![WritingStyleRevealPage::Read, WritingStyleRevealPage::Name],
            languages: Vec::new(),
            name: "My style".to_owned(),
            notes: String::new(),
            save: FREE,
            close: FREE,
            paging: FREE,
        },
        2,
        Rc::new(|_| {}),
    );
    let outcome = |serial, result| WritingStyleOutcome {
        serial,
        kind: WritingStyleChangeKind::SaveReveal,
        style: Some("style-1".to_owned()),
        account_id: None,
        result,
    };
    let done = outcome(2, WritingStyleResult::Done);
    assert!(
        !reveal.outcome(Some(&done)),
        "one from before it opened says nothing"
    );
    let refused = outcome(
        3,
        WritingStyleResult::Refused {
            reason: WritingStyleRefusal::BlankName,
        },
    );
    assert!(!reveal.outcome(Some(&refused)));
    assert!(!reveal.outcome(Some(&refused)), "and is acted on once");
    assert!(reveal.outcome(Some(&outcome(4, WritingStyleResult::Done))));
    let step = reveal.step();
    let primary = step.primary.expect("Next");
    assert_eq!(primary.label, l10n::wizard_next());
}

/// What a flow intent asked for, for the record of what was pressed.
fn said(intent: &StyleLearningIntent) -> String {
    match intent {
        StyleLearningIntent::SelectAccount { account } => format!("SelectAccount {account}"),
        StyleLearningIntent::Learn { name, language } => format!("Learn {name} {language}"),
        StyleLearningIntent::Stop => "Stop".to_owned(),
        StyleLearningIntent::Close => "Close".to_owned(),
        StyleLearningIntent::Remedy => "Remedy".to_owned(),
        StyleLearningIntent::Next => "Next".to_owned(),
        StyleLearningIntent::Back => "Back".to_owned(),
        StyleLearningIntent::Open => "Open".to_owned(),
        StyleLearningIntent::SelectRange { .. } => "SelectRange".to_owned(),
    }
}
