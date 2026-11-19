//! The flow's remedies, a review with nothing to send and the reveal's letter as a screen reader
//! hears it (`docs/ai.md` rules 11, 15 and 27). Run from [`super::all`].

use adw::prelude::*;
use mailcal_bindings::{
    AiFailure, AiRemedy, AiRemedyControl, StyleLearnRange, StyleLearningReport,
    StyleLearningReview, StyleLearningStage, StyleLearningTrailStage, StyleSentReach,
    WritingStyleLearning, WritingStyleLetter, WritingStyleParagraph, WritingStyleSpan,
};

use super::{Asked, FREE, HELD, button, flow, on_screen, record};
use crate::{
    l10n,
    ui::{
        mailbox::tests::rendered_labels,
        settings::{writing_style_reveal_pages as pages, writing_style_words as words},
    },
};

pub(super) fn every_remedy_is_sent_alone_and_one_leading_elsewhere_opened() {
    for (remedy, expected) in [
        (AiRemedy::ChooseRange, vec!["Remedy", "open ChooseRange"]),
        (
            AiRemedy::OpenAiEndpoint,
            vec!["Remedy", "open OpenAiEndpoint"],
        ),
        (
            AiRemedy::SignInToAllodia,
            vec!["Remedy", "open SignInToAllodia"],
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

pub(super) fn nothing_usable_names_no_destination() {
    let flow = flow(&Asked::default());
    flow.draw(
        WritingStyleLearning {
            back: Some(FREE),
            learn: Some(HELD),
            ..record(StyleLearningStage::Review {
                email: "alice@test.local".to_owned(),
                range: StyleLearnRange::Everything,
                reach: StyleSentReach::Everything,
                state: StyleLearningReview::NothingUsable {
                    report: StyleLearningReport {
                        found: 3,
                        usable: 0,
                        languages: Vec::new(),
                        other_languages: 0,
                        oldest: None,
                        newest: None,
                    },
                },
                sends_to: None,
                cannot_send: None,
                keyword_kept: true,
            })
        },
        None,
    );
    let shown = rendered_labels(&on_screen(&flow));
    assert!(
        shown
            .iter()
            .any(|text| text == l10n::learn_report_nothing())
    );
    for unsaid in [
        l10n::learn_consent_title(),
        l10n::learn_consent_relay(),
        l10n::ai_failure_nowhere_to_send(),
    ] {
        assert!(
            !shown.iter().any(|text| text == unsaid),
            "nothing is sent, so nothing is said about where: {unsaid}"
        );
    }
}

pub(super) fn the_letter_is_spoken_as_one_sentence() {
    let span = |text: &str, placeholder| WritingStyleSpan {
        text: text.to_owned(),
        placeholder,
    };
    let mut letter = WritingStyleLetter {
        opening: vec![span("Hi ", false), span("Name", true), span(",", false)],
        paragraphs: vec![WritingStyleParagraph {
            lines: vec![1.0, 0.5],
        }],
        closing: vec![span("Best,", false)],
        signs_as: "Alice".to_owned(),
        words: 70,
        paragraph_count: 3,
        shape: String::new(),
    };
    assert_eq!(
        words::letter(&letter),
        [
            l10n::a11y_letter_opening("Hi Name,"),
            l10n::a11y_letter_words(70),
            l10n::a11y_letter_paragraphs(3),
            l10n::a11y_letter_closing("Best,"),
            l10n::a11y_letter_name("Alice"),
        ]
        .join(", ")
    );
    letter.opening.clear();
    letter.words = 0;
    letter.paragraph_count = 0;
    letter.signs_as.clear();
    assert_eq!(
        words::letter(&letter),
        l10n::a11y_letter_closing("Best,"),
        "an empty or zero value is left out, the lines drawn never counted"
    );
    let body = gtk::Box::new(gtk::Orientation::Vertical, 0);
    pages::letter(&body, &letter);
    let paper = body
        .first_child()
        .and_then(|column| column.first_child())
        .expect("the letter");
    assert_eq!(paper.accessible_role(), gtk::AccessibleRole::Img);
}
