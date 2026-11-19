//! The learning flow on the showcase under `MAILCAL_OFFER_AI`: the first account's review is
//! `Ready` in two languages and its run ends unreachable, the second's has nothing usable, and a
//! showcase without the hook is the seed unchanged.

use std::{
    sync::{Arc, mpsc},
    time::{Duration, Instant},
};

use crate::{
    AiFailure, AiIntent, Intent, LogLevel, MailcalApp, ShowcaseLocale, StyleLearningDestination,
    StyleLearningIntent, StyleLearningReview, StyleLearningStage, WritingStyleIntent,
    WritingStyleLearning, device_zone, showcase_data,
    tests::{ChannelObserver, NullLogger},
};

/// A showcase built as a launch with the hook or without it builds one, with the assistant
/// offered and its styles staged either way.
fn showcase(staged_sent_mail: bool) -> Arc<MailcalApp> {
    let (tx, _rx) = mpsc::channel();
    let app = super::super::super::inmemory::build_showcase_with(
        Box::new(ChannelObserver { tx }),
        Box::new(NullLogger),
        LogLevel::Info,
        "Europe/Amsterdam".to_owned(),
        ShowcaseLocale::En,
        staged_sent_mail,
    );
    let account = app.app.account_ids()[0].as_str().to_owned();
    super::super::offer_ai(&app);
    super::stage(app.app.as_ref(), &account, &account);
    app
}

fn press(app: &MailcalApp, intent: StyleLearningIntent) {
    app.dispatch(Intent::Ai {
        intent: AiIntent::WritingStyle {
            intent: WritingStyleIntent::Learning { intent },
        },
    });
}

fn eventually(
    app: &MailcalApp,
    mut found: impl FnMut(&WritingStyleLearning) -> bool,
) -> WritingStyleLearning {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(flow) = app.writing_style_learning().filter(|flow| found(flow)) {
            return flow;
        }
        assert!(
            Instant::now() < deadline,
            "never came: {:?}",
            app.writing_style_learning()
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// Opens the flow, reads `account`'s whole Sent folder and returns the review once it is read.
fn review(app: &MailcalApp, account: &str) -> WritingStyleLearning {
    press(app, StyleLearningIntent::Open);
    press(
        app,
        StyleLearningIntent::SelectAccount {
            account: account.to_owned(),
        },
    );
    press(app, StyleLearningIntent::Next);
    eventually(app, |flow| {
        matches!(flow.stage, StyleLearningStage::ChooseRange { .. })
    });
    press(app, StyleLearningIntent::Next);
    eventually(app, |flow| {
        matches!(
            flow.stage,
            StyleLearningStage::Review {
                state: StyleLearningReview::Ready { .. }
                    | StyleLearningReview::NothingUsable { .. },
                ..
            }
        )
    })
}

#[test]
fn the_first_account_learns_until_unreachable_and_the_second_has_nothing_usable() {
    let app = showcase(true);
    let accounts = app.app.account_ids();
    let ready = review(&app, accounts[0].as_str());
    let StyleLearningStage::Review {
        state: StyleLearningReview::Ready { report },
        sends_to,
        cannot_send,
        ..
    } = ready.stage
    else {
        panic!(
            "the first account has mail to learn from: {:?}",
            ready.stage
        );
    };
    let languages: Vec<&str> = report
        .languages
        .iter()
        .map(|language| language.language.as_str())
        .collect();
    assert!(
        languages.contains(&"en") && languages.contains(&"nl"),
        "{languages:?}"
    );
    assert!(
        matches!(sends_to, Some(StyleLearningDestination::OwnEndpoint { .. })),
        "the staged endpoint on this device"
    );
    assert_eq!(cannot_send, None);
    assert!(ready.learn.is_some_and(|learn| learn.enabled));

    press(
        &app,
        StyleLearningIntent::Learn {
            name: "My style".to_owned(),
            language: "en".to_owned(),
        },
    );
    let failed = eventually(&app, |flow| {
        matches!(flow.stage, StyleLearningStage::Failed { .. })
    });
    assert!(matches!(
        failed.stage,
        StyleLearningStage::Failed {
            failure: AiFailure::Unreachable,
            ..
        }
    ));

    press(&app, StyleLearningIntent::Close);
    let nothing = review(&app, accounts[1].as_str());
    assert!(
        matches!(
            nothing.stage,
            StyleLearningStage::Review {
                state: StyleLearningReview::NothingUsable { .. },
                sends_to: None,
                cannot_send: None,
                ..
            }
        ),
        "{:?}",
        nothing.stage
    );
    assert!(nothing.learn.is_some_and(|learn| !learn.enabled));
}

#[test]
fn a_showcase_without_the_hook_is_the_seed_unchanged() {
    let now = showcase_data::seeded_now(&device_zone("Europe/Amsterdam".to_owned()));
    for locale in [
        ShowcaseLocale::En,
        ShowcaseLocale::Nl,
        ShowcaseLocale::De,
        ShowcaseLocale::Fr,
        ShowcaseLocale::Es,
        ShowcaseLocale::It,
        ShowcaseLocale::Pt,
    ] {
        let seeded = showcase_data::primary(locale, now);
        let kept = super::super::primary_seed(showcase_data::primary(locale, now), now, false);
        assert_eq!(kept.messages, seeded.messages, "{locale:?}");
        assert_eq!(kept.bodies, seeded.bodies, "{locale:?}");
        assert_eq!(kept.mailboxes, seeded.mailboxes, "{locale:?}");

        let staged = super::super::primary_seed(showcase_data::primary(locale, now), now, true);
        assert_eq!(staged.messages[..seeded.messages.len()], seeded.messages);
        assert!(
            staged.messages[seeded.messages.len()..]
                .iter()
                .all(|message| message.id.key().as_str().starts_with("ai-sent-")),
            "the hook only adds"
        );
    }

    let app = showcase(false);
    let first = app.app.account_ids()[0].as_str().to_owned();
    let unchanged = review(&app, &first);
    assert!(
        matches!(
            unchanged.stage,
            StyleLearningStage::Review {
                state: StyleLearningReview::NothingUsable { .. },
                ..
            }
        ),
        "without the hook the seeded sent mail is too short to learn from"
    );
}
