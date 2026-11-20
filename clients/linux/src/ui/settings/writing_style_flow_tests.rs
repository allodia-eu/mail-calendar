//! The range and what it fetches, the review while it fetches, the flow's remedies, a review with
//! nothing to send and the reveal's letter as a screen reader hears it (`docs/ai.md` rules 11, 15
//! and 27). Run from [`super::all`].

use adw::prelude::*;
use mailcal_bindings::{
    AiFailure, AiRemedy, AiRemedyControl, StyleLearnRange, StyleLearningDestination,
    StyleLearningReport, StyleLearningReview, StyleLearningStage, StyleLearningTrailStage,
    StyleRangeFetch, StyleSentReach, WritingStyleLearning, WritingStyleLetter,
    WritingStyleParagraph, WritingStyleSpan,
};

use super::{Asked, FREE, HELD, button, descendants, flow, on_screen, record};
use crate::{
    l10n,
    ui::{
        mailbox::tests::rendered_labels,
        settings::{writing_style_reveal_pages as pages, writing_style_words as words},
    },
};

fn choose_range(selected: StyleLearnRange, fetch: StyleRangeFetch) -> WritingStyleLearning {
    WritingStyleLearning {
        next: Some(FREE),
        trail: vec![StyleLearningTrailStage::ChooseAccount],
        ..record(StyleLearningStage::ChooseRange {
            email: "alice@test.local".to_owned(),
            presets: vec![
                StyleLearnRange::LastTwelveMonths,
                StyleLearnRange::Everything,
                StyleLearnRange::UntilEndOfLastYear,
                StyleLearnRange::Between {
                    since: "2025-11-20".to_owned(),
                    until: "2026-11-20".to_owned(),
                },
            ],
            latest: "2026-11-20".to_owned(),
            selected: Some(selected),
            reach: StyleSentReach::Since {
                day: "2026-08-20".to_owned(),
            },
            fetch,
        })
    }
}

pub(super) fn the_range_says_what_next_fetches() {
    let asked = Asked::default();
    let flow = flow(&asked);
    flow.draw(
        choose_range(StyleLearnRange::LastTwelveMonths, StyleRangeFetch::Older),
        None,
    );
    let page = on_screen(&flow);
    let shown = rendered_labels(&page);
    assert!(
        shown
            .iter()
            .any(|text| text == l10n::learn_range_last_twelve_months())
    );
    assert!(
        descendants::<gtk::CheckButton>(&page)[0].is_active(),
        "the first preset is the one selected"
    );
    assert!(shown.iter().any(|text| text == l10n::learn_range_fetch_older()));
    assert!(!shown.iter().any(|text| text == l10n::learn_range_fetch_all()));

    flow.draw(
        choose_range(StyleLearnRange::Everything, StyleRangeFetch::WholeHistory),
        None,
    );
    let shown = rendered_labels(&on_screen(&flow));
    assert!(shown.iter().any(|text| text == l10n::learn_range_fetch_all()));
    assert!(!shown.iter().any(|text| text == l10n::learn_range_fetch_older()));

    flow.draw(
        choose_range(StyleLearnRange::UntilEndOfLastYear, StyleRangeFetch::Nothing),
        None,
    );
    let shown = rendered_labels(&on_screen(&flow));
    for unsaid in [l10n::learn_range_fetch_all(), l10n::learn_range_fetch_older()] {
        assert!(!shown.iter().any(|text| text == unsaid), "{unsaid}");
    }
    assert!(asked.borrow().is_empty(), "drawing sends nothing");
}

fn review(state: StyleLearningReview, older_not_fetched: bool) -> WritingStyleLearning {
    WritingStyleLearning {
        back: Some(FREE),
        learn: Some(HELD),
        trail: vec![StyleLearningTrailStage::ChooseRange],
        ..record(StyleLearningStage::Review {
            email: "alice@test.local".to_owned(),
            range: StyleLearnRange::LastTwelveMonths,
            reach: StyleSentReach::Since {
                day: "2026-08-20".to_owned(),
            },
            older_not_fetched,
            state,
            sends_to: Some(StyleLearningDestination::Relay),
            cannot_send: None,
            keyword_kept: true,
        })
    }
}

pub(super) fn the_review_fetches_with_a_spinner_and_says_what_it_could_not() {
    let flow = flow(&Asked::default());
    flow.draw(review(StyleLearningReview::Fetching, false), None);
    let page = on_screen(&flow);
    assert!(
        rendered_labels(&page)
            .iter()
            .any(|text| text == l10n::learn_fetching())
    );
    assert!(
        flow.host.widget().visible_page().expect("a page").can_pop(),
        "Back stays as the record gives it while the fetch runs"
    );

    let report = StyleLearningReport {
        found: 8,
        usable: 6,
        languages: Vec::new(),
        other_languages: 0,
        oldest: None,
        newest: None,
    };
    flow.draw(
        review(
            StyleLearningReview::Ready {
                report: report.clone(),
            },
            true,
        ),
        None,
    );
    assert!(
        rendered_labels(&on_screen(&flow))
            .iter()
            .any(|text| text == l10n::learn_older_not_fetched())
    );
    flow.draw(review(StyleLearningReview::Ready { report }, false), None);
    assert!(
        !rendered_labels(&on_screen(&flow))
            .iter()
            .any(|text| text == l10n::learn_older_not_fetched())
    );
}

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
                older_not_fetched: false,
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
            // The comma joining the parts stands in for the one the greeting and sign-off end on.
            l10n::a11y_letter_opening("Hi Name"),
            l10n::a11y_letter_words(70),
            l10n::a11y_letter_paragraphs(3),
            l10n::a11y_letter_closing("Best"),
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
        l10n::a11y_letter_closing("Best"),
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
