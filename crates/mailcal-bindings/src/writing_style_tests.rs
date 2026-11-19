//! Settings → Writing style as a host reaches it: the page and its blocked Learn, library changes
//! through `dispatch` running off the host's thread, and a style's reveal and detail as records.

use std::{
    collections::BTreeMap,
    sync::{Arc, mpsc},
    time::{Duration, Instant},
};

use mailcal_account::StoredWritingStyle;
use mailcal_ai::{Habit, LanguageCode, LanguageStyle, Provenance, StyleGuide};

use crate::{
    AiIntent, Intent, LogLevel, MailcalApp, StyleLearningIntent, WritingStyleIntent,
    WritingStyleLearnBlock, WritingStyleOpens, WritingStylePage, WritingStyleRefusal,
    WritingStyleResult, WritingStyleRevealPage, WritingStyleSpan,
    tests::{ChannelObserver, NullLogger, RecordingCredentialStore, RecordingStoreHandle},
    writing_style_name_refusal,
};

fn app(name: &str) -> Arc<MailcalApp> {
    let (tx, _rx) = mpsc::channel();
    let dir = crate::tests::temp_data_dir(name);
    MailcalApp::new_accounts(
        Box::new(ChannelObserver { tx }),
        Box::new(NullLogger),
        LogLevel::Info,
        Vec::new(),
        dir.to_string_lossy().into_owned(),
        "Etc/UTC".to_owned(),
        crate::analytics::test_device(),
        Box::new(RecordingStoreHandle(Arc::new(
            RecordingCredentialStore::default(),
        ))),
    )
    .expect("the app boots")
}

fn stored() -> StoredWritingStyle {
    let en = LanguageCode::new("en").unwrap();
    let mut guide = StyleGuide::new();
    guide.languages.insert(
        en.clone(),
        LanguageStyle {
            greetings: vec![Habit {
                text: "Dear [Name],".to_owned(),
                share: 55,
            }],
            ..LanguageStyle::default()
        },
    );
    guide.learned = Some(Provenance {
        messages_per_language: BTreeMap::from([(en, 9)]),
        ..Provenance::default()
    });
    StoredWritingStyle {
        name: "Work".to_owned(),
        learned_from: "me@example.eu".to_owned(),
        guide_json: serde_json::to_string(&guide).unwrap(),
        exemplars_json: String::new(),
        revealed: false,
    }
}

fn send(app: &MailcalApp, intent: WritingStyleIntent) {
    app.dispatch(Intent::Ai {
        intent: AiIntent::WritingStyle { intent },
    });
}

/// The page once a change after `serial` has ended.
fn page_after(app: &MailcalApp, serial: u64) -> WritingStylePage {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let page = app.writing_style_page().expect("offered");
        if page
            .outcome
            .as_ref()
            .is_some_and(|outcome| outcome.serial > serial)
        {
            return page;
        }
        assert!(Instant::now() < deadline, "the change never ended");
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn without_an_account_used_for_mail_learn_says_so_and_opens_nothing() {
    let app = app("ws-no-account");
    let page = app
        .writing_style_page()
        .expect("a development build offers it");
    assert_eq!(
        page.learn_blocked,
        Some(WritingStyleLearnBlock::NoMailAccount)
    );
    assert!(!page.learn.enabled);
    send(
        &app,
        WritingStyleIntent::Learning {
            intent: StyleLearningIntent::Open,
        },
    );
    assert!(app.writing_style_learning().is_none());
}

#[test]
fn a_rename_runs_off_the_hosts_thread_and_a_blank_one_is_refused_by_the_rule_the_field_asks() {
    let app = app("ws-rename");
    let id = app.app.add_writing_style(None, stored());
    assert_eq!(
        writing_style_name_refusal("  ".to_owned()),
        Some(WritingStyleRefusal::BlankName)
    );
    assert_eq!(writing_style_name_refusal("Work".to_owned()), None);

    send(
        &app,
        WritingStyleIntent::Rename {
            style: id.as_str().to_owned(),
            name: " ".to_owned(),
        },
    );
    let page = page_after(&app, 0);
    assert_eq!(
        page.outcome.unwrap().result,
        WritingStyleResult::Refused {
            reason: WritingStyleRefusal::BlankName
        }
    );
    send(
        &app,
        WritingStyleIntent::Rename {
            style: id.as_str().to_owned(),
            name: "Replies".to_owned(),
        },
    );
    let page = page_after(&app, 1);
    assert_eq!(page.outcome.unwrap().result, WritingStyleResult::Done);
    assert_eq!(page.styles[0].name, "Replies");
    assert_eq!(page.styles[0].opens, WritingStyleOpens::Reveal);
}

#[test]
fn closing_the_reveal_counts_it_as_shown_and_forget_empties_the_library() {
    let app = app("ws-reveal");
    let id = app.app.add_writing_style(None, stored());
    let reveal = app
        .writing_style_reveal(id.as_str().to_owned())
        .expect("a reveal");
    assert_eq!(
        reveal.pages,
        [
            WritingStyleRevealPage::Read,
            WritingStyleRevealPage::Letter,
            WritingStyleRevealPage::Habits,
            WritingStyleRevealPage::Name
        ]
    );
    assert_eq!(reveal.facts.messages, 9);
    let letter = reveal.languages[0].letter.as_ref().unwrap();
    assert_eq!(
        letter.opening[1],
        WritingStyleSpan {
            text: "Name".to_owned(),
            placeholder: true
        }
    );
    assert_eq!(letter.paragraphs.len(), 2);

    send(
        &app,
        WritingStyleIntent::CloseReveal {
            style: id.as_str().to_owned(),
        },
    );
    let page = page_after(&app, 0);
    assert_eq!(page.styles[0].opens, WritingStyleOpens::Detail);
    let detail = app
        .writing_style_detail(id.as_str().to_owned())
        .expect("a detail");
    assert!(detail.forget.enabled);

    send(
        &app,
        WritingStyleIntent::Forget {
            style: id.as_str().to_owned(),
        },
    );
    let page = page_after(&app, 1);
    assert!(page.styles.is_empty());
    assert!(app.writing_style_detail(id.as_str().to_owned()).is_none());
}
