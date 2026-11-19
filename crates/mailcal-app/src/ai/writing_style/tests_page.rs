//! Settings → Writing style, its library changes, and a style's reveal and detail.

use std::collections::BTreeMap;

use mailcal_account::{StoredWritingStyle, WritingStyleId};
use mailcal_ai::{Habit, LanguageCode, LanguageStyle, Provenance, StyleGuide};
use mailcal_viewmodel::writing_style::{RevealPage, RevealTopic};

use super::{
    LearnBlock, StyleOpens, WritingStyleChange, WritingStyleChangeKind, WritingStyleRefusal,
    WritingStyleResult, writing_style_name,
};
use crate::ai::{AiEntitlement, AiFailure, BuildKind, tests_support::Fixture};

fn code(code: &str) -> LanguageCode {
    LanguageCode::new(code).unwrap()
}

/// A style learned from 12 Dutch and 30 English messages.
fn stored() -> StoredWritingStyle {
    let english = LanguageStyle {
        greetings: vec![Habit {
            text: "Hi [Name],".to_owned(),
            share: 70,
        }],
        signs_as: "Sam".to_owned(),
        register: "Informal and warm. Uses first names.".to_owned(),
        structure_headline: "Thanks first".to_owned(),
        structure: "Opens with thanks.".to_owned(),
        typical_words: 80,
        ..LanguageStyle::default()
    };
    let dutch = LanguageStyle {
        phrases: vec!["Laat maar weten".to_owned()],
        ..LanguageStyle::default()
    };
    let mut guide = StyleGuide::new();
    guide.languages.insert(code("nl"), dutch);
    guide.languages.insert(code("en"), english);
    guide.notes = "Never use exclamation marks.".to_owned();
    guide.learned = Some(Provenance {
        oldest: Some(1_760_000_000),
        newest: Some(1_763_000_000),
        messages_per_language: BTreeMap::from([(code("en"), 30), (code("nl"), 12)]),
        learned_at: 1_763_500_000,
        ..Provenance::default()
    });
    StoredWritingStyle {
        name: "Work".to_owned(),
        learned_from: "me@acct-1.local".to_owned(),
        guide_json: serde_json::to_string(&guide).unwrap(),
        exemplars_json: String::new(),
        revealed: false,
    }
}

fn with_style() -> (Fixture, WritingStyleId) {
    let fixture = Fixture::new(BuildKind::Development);
    let id = fixture.app.add_writing_style(Some("acct-1"), stored());
    (fixture, id)
}

#[test]
fn the_page_is_drawn_only_while_the_assistant_is_offered() {
    let fixture = Fixture::new(BuildKind::Production);
    assert!(fixture.app.writing_style_page().is_none());
    fixture.app.set_ai_entitlement(AiEntitlement::Granted);
    assert!(fixture.app.writing_style_page().is_some());
}

#[test]
fn learn_says_why_it_cannot_be_pressed() {
    let fixture = Fixture::new(BuildKind::Development);
    let page = fixture.app.writing_style_page().unwrap();
    assert_eq!(
        page.learn_blocked,
        Some(LearnBlock::CannotSend(AiFailure::NowhereToSend))
    );
    assert!(!page.learn.enabled);
    assert_eq!(page.styles.len(), 0);
    assert!(
        page.accounts.is_empty(),
        "no slot pickers while the library is empty"
    );
}

#[test]
fn a_style_is_a_row_and_every_account_used_for_mail_a_slot() {
    let (fixture, id) = with_style();
    let page = fixture.app.writing_style_page().unwrap();
    let row = &page.styles[0];
    assert_eq!(row.name, "Work");
    assert_eq!(row.facts.messages, 42);
    assert_eq!(row.facts.languages, ["en", "nl"], "most messages first");
    assert_eq!(row.facts.learned_from, "me@acct-1.local");
    assert_eq!(
        row.opens,
        StyleOpens::Reveal,
        "the reveal has not been shown"
    );
    assert_eq!(page.accounts.len(), 1);
    assert_eq!(page.accounts[0].selected, Some(id.clone()));
    assert!(page.accounts[0].picker.enabled);
}

#[test]
fn a_change_holds_the_page_and_runs_its_own_control() {
    let (fixture, id) = with_style();
    let app = &fixture.app;
    assert!(app.queue_writing_style_change(WritingStyleChange::Assign {
        account: "acct-1".to_owned(),
        style: None,
    }));
    assert!(
        !app.queue_writing_style_change(WritingStyleChange::Assign {
            account: "acct-1".to_owned(),
            style: None,
        }),
        "the same change asked twice is dropped"
    );
    assert!(!app.queue_writing_style_change(WritingStyleChange::Forget { id: id.clone() }));
    let page = app.writing_style_page().unwrap();
    assert!(page.accounts[0].picker.running);
    assert!(!page.styles[0].open.enabled);
    assert!(!page.learn.enabled);
    let detail = app.writing_style_detail(&id).unwrap();
    assert!(
        detail.forget.running,
        "a change waiting is drawn running from then (docs/waiting.md rule 2)"
    );
    assert!(!detail.forget.enabled);

    app.run_writing_style_changes();
    let page = app.writing_style_page().unwrap();
    assert!(page.styles.is_empty(), "both ran, one after the other");
    let outcome = page.outcome.unwrap();
    assert_eq!(outcome.work.kind, WritingStyleChangeKind::Forget);
    assert_eq!(outcome.result, WritingStyleResult::Done);
    assert_eq!(app.writing_style_slot("acct-1"), None);
}

#[test]
fn a_blank_name_is_refused_by_the_rule_the_editor_asks() {
    assert_eq!(
        writing_style_name("  "),
        Err(WritingStyleRefusal::BlankName)
    );
    assert_eq!(writing_style_name(" Work "), Ok("Work".to_owned()));
    let (fixture, id) = with_style();
    fixture
        .app
        .change_writing_style_now(WritingStyleChange::Rename {
            id: id.clone(),
            name: " ".to_owned(),
        });
    let page = fixture.app.writing_style_page().unwrap();
    assert_eq!(
        page.outcome.unwrap().result,
        WritingStyleResult::Refused {
            reason: WritingStyleRefusal::BlankName
        }
    );
    assert_eq!(page.styles[0].name, "Work");

    let gone = WritingStyleId::new("gone").unwrap();
    fixture
        .app
        .change_writing_style_now(WritingStyleChange::Notes {
            id: gone,
            notes: "x".to_owned(),
        });
    assert_eq!(
        fixture
            .app
            .writing_style_page()
            .unwrap()
            .outcome
            .unwrap()
            .result,
        WritingStyleResult::Refused {
            reason: WritingStyleRefusal::Gone
        }
    );
}

#[test]
fn saving_the_reveal_keeps_the_name_and_notes_and_opens_the_detail_from_then_on() {
    let (fixture, id) = with_style();
    fixture
        .app
        .change_writing_style_now(WritingStyleChange::SaveReveal {
            id: id.clone(),
            name: " Replies ".to_owned(),
            notes: " Keep it short. ".to_owned(),
        });
    let detail = fixture.app.writing_style_detail(&id).unwrap();
    assert_eq!(detail.name, "Replies");
    assert_eq!(detail.notes, "Keep it short.");
    assert_eq!(detail.facts.messages, 42, "the description is kept");
    let page = fixture.app.writing_style_page().unwrap();
    assert_eq!(page.styles[0].opens, StyleOpens::Detail);
}

#[test]
fn forget_is_asked_naming_the_accounts_that_draft_in_the_style() {
    let (fixture, id) = with_style();
    let detail = fixture.app.writing_style_detail(&id).unwrap();
    assert_eq!(detail.drafting_accounts, ["me@acct-1.local"]);
    assert!(detail.forget.enabled);
    fixture
        .app
        .change_writing_style_now(WritingStyleChange::Assign {
            account: "acct-1".to_owned(),
            style: None,
        });
    let detail = fixture.app.writing_style_detail(&id).unwrap();
    assert_eq!(detail.drafting_accounts.len(), 0);
}

#[test]
fn the_reveal_leaves_out_what_was_not_noticed_and_orders_languages_by_messages() {
    let (fixture, id) = with_style();
    let reveal = fixture.app.writing_style_reveal(&id).unwrap();
    assert_eq!(
        reveal.pages,
        [
            RevealPage::Read,
            RevealPage::Letter,
            RevealPage::Habits,
            RevealPage::Voice,
            RevealPage::Phrases,
            RevealPage::Name
        ]
    );
    assert_eq!(reveal.facts.since, Some(1_760_000_000));
    assert_eq!(reveal.notes, "Never use exclamation marks.");
    let [english, dutch] = &reveal.languages[..] else {
        panic!("two languages");
    };
    assert_eq!(english.language, "en");
    let letter = english.letter.as_ref().unwrap();
    assert_eq!(letter.opening[1].text, "Name");
    assert!(letter.opening[1].placeholder);
    assert_eq!(letter.signs_as, "Sam");
    let cards: Vec<(RevealTopic, &str, &str)> = english
        .cards
        .iter()
        .map(|card| (card.topic, card.heading.as_str(), card.body.as_str()))
        .collect();
    assert_eq!(
        cards,
        [
            (
                RevealTopic::Register,
                "Informal and warm",
                "Uses first names."
            ),
            (RevealTopic::Structure, "Thanks first", "Opens with thanks."),
        ]
    );
    assert!(english.phrases.is_none());
    assert!(dutch.letter.is_none() && dutch.habits.is_none() && dutch.cards.is_empty());
    assert_eq!(dutch.phrases.as_ref().unwrap().used, ["Laat maar weten"]);
    assert!(reveal.save.enabled && reveal.close.enabled);
}
