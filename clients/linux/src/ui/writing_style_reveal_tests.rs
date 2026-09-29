//! The reveal's steps and the arithmetic behind its pictures, pinned without a window. Each rule is
//! one a page gets wrong while looking right: a letter whose lines ignore the paragraph count, a
//! card that repeats its heading as its first sentence, a placeholder drawn with its brackets.

use mailcal_bindings::{
    AccountWritingStyleRow, HabitFrequency, LanguageStyleRow, WritingStyleDetail, WritingStyleRow,
};

use super::{
    RevealRun, RevealStep, WizardPager, card_text, frequency_text, letter_lines, reveal_runs,
    shows_languages, source_address, stats_label,
};
use crate::{
    l10n,
    ui::{timestamps, writing_style::languages_text},
};

#[test]
fn the_language_control_is_on_the_four_pages_about_one_language() {
    let per_language = RevealStep::ALL
        .into_iter()
        .filter(|step| step.is_per_language())
        .collect::<Vec<_>>();
    assert_eq!(
        per_language,
        [
            RevealStep::Letter,
            RevealStep::Habits,
            RevealStep::Voice,
            RevealStep::Phrases
        ]
    );
    assert!(shows_languages(RevealStep::Voice, 2));
    assert!(
        !shows_languages(RevealStep::Voice, 1),
        "one language has nothing to choose between"
    );
    assert!(!shows_languages(RevealStep::Name, 3));
}

#[test]
fn the_pager_stays_within_its_pages() {
    let mut pager = WizardPager::new(6);
    assert!(pager.is_first() && !pager.is_last());
    pager.back();
    assert_eq!(pager.index(), 0);
    pager.go(5);
    assert!(pager.is_last());
    pager.go(9);
    assert_eq!(pager.index(), 5);
    pager.next();
    assert_eq!(pager.index(), 5);
    pager.back();
    assert_eq!(pager.index(), 4);
    assert_eq!(pager.step_label(), l10n::a11y_wizard_step("5", "6"));
    assert!(
        WizardPager::new(0).is_last(),
        "a sheet has at least one page"
    );
}

/// Seventy words in two paragraphs is about five lines, the longer paragraph first, and each
/// paragraph ends on a short line.
#[test]
fn a_letter_draws_its_words_at_about_fifteen_to_a_line() {
    let lines = letter_lines(70, 2);
    assert_eq!(lines.iter().map(Vec::len).collect::<Vec<_>>(), [3, 2]);
    for paragraph in &lines {
        let (last, rest) = paragraph.split_last().expect("a line at least");
        assert!(*last < 0.8);
        assert!(rest.iter().all(|width| *width > 0.9));
    }
}

#[test]
fn a_letter_without_a_paragraph_count_draws_two() {
    assert_eq!(letter_lines(90, 0).len(), 2);
    assert_eq!(
        letter_lines(0, 0).iter().map(Vec::len).collect::<Vec<_>>(),
        [2, 2]
    );
}

#[test]
fn a_letter_has_a_line_for_every_paragraph_and_room_for_all() {
    assert_eq!(
        letter_lines(10, 3).iter().map(Vec::len).collect::<Vec<_>>(),
        [1, 1, 1]
    );
    assert_eq!(letter_lines(900, 3).iter().map(Vec::len).sum::<usize>(), 12);
    assert_eq!(letter_lines(200, 40).len(), 6);
}

#[test]
fn a_card_keeps_the_cores_heading_over_the_whole_description() {
    let (headline, body) = card_text(" Friendly and direct ", "On first-name terms. Brief.");
    assert_eq!(headline, "Friendly and direct");
    assert_eq!(body, "On first-name terms. Brief.");
}

#[test]
fn a_card_without_a_heading_leads_with_its_first_sentence_once() {
    assert_eq!(
        card_text("", "Thanks first, then the answer. Short lines."),
        (
            "Thanks first, then the answer".to_owned(),
            "Short lines.".to_owned()
        )
    );
    assert_eq!(
        card_text("", "Says no politely."),
        ("Says no politely".to_owned(), String::new())
    );
    assert_eq!(card_text("", "  "), (String::new(), String::new()));
    assert_eq!(
        card_text("", "Keeps 3.5 hours free. Then works."),
        ("Keeps 3.5 hours free".to_owned(), "Then works.".to_owned()),
        "a decimal point ends no sentence"
    );
}

fn words(text: &str) -> RevealRun {
    RevealRun {
        text: text.to_owned(),
        placeholder: false,
    }
}

fn placeholder(text: &str) -> RevealRun {
    RevealRun {
        text: text.to_owned(),
        placeholder: true,
    }
}

#[test]
fn a_placeholder_is_a_run_of_its_own_without_its_brackets() {
    assert_eq!(
        reveal_runs("Hi [Name],"),
        [words("Hi "), placeholder("Name"), words(",")]
    );
    assert_eq!(reveal_runs("Cheers,"), [words("Cheers,")]);
    assert_eq!(reveal_runs("[Name]"), [placeholder("Name")]);
    assert_eq!(reveal_runs("Hi [] all"), [words("Hi [] all")]);
    assert_eq!(
        reveal_runs("Dear [[Name]"),
        [words("Dear ["), placeholder("Name")]
    );
}

#[test]
fn each_frequency_reads_its_own_key() {
    assert_eq!(
        frequency_text(HabitFrequency::Mostly),
        l10n::reveal_frequency_mostly()
    );
    assert_eq!(
        frequency_text(HabitFrequency::Often),
        l10n::reveal_frequency_often()
    );
    assert_eq!(
        frequency_text(HabitFrequency::Sometimes),
        l10n::reveal_frequency_sometimes()
    );
}

/// The source is an account id, and the line names its address only while that account is here.
#[test]
fn the_source_line_names_an_account_still_on_this_device() {
    let accounts = [AccountWritingStyleRow {
        account_id: "work".to_owned(),
        email: "alice@test.local".to_owned(),
        style: None,
    }];
    assert_eq!(source_address("work", &accounts), Some("alice@test.local"));
    assert_eq!(source_address("gone", &accounts), None);
    assert_eq!(
        source_address("", &accounts),
        None,
        "a style from another device"
    );
}

fn detail(oldest: Option<i64>) -> WritingStyleDetail {
    let language = |code: &str| LanguageStyleRow {
        language: code.to_owned(),
        greetings: Vec::new(),
        sign_offs: Vec::new(),
        signs_as: String::new(),
        register: String::new(),
        register_headline: String::new(),
        typical_words: 0,
        typical_paragraphs: 0,
        shape: String::new(),
        punctuation: String::new(),
        structure: String::new(),
        structure_headline: String::new(),
        moves: String::new(),
        moves_headline: String::new(),
        phrases: Vec::new(),
        avoid: Vec::new(),
    };
    WritingStyleDetail {
        row: WritingStyleRow {
            id: "style".to_owned(),
            name: "My style".to_owned(),
            source_account: "work".to_owned(),
            languages: vec!["en".to_owned(), "nl".to_owned()],
            messages: 214,
            oldest,
            newest: None,
            learned_at: 0,
        },
        notes: String::new(),
        languages: vec![language("en"), language("nl")],
    }
}

/// The figures are read as one sentence only when there is a date to say it with.
#[test]
fn the_figures_read_as_one_sentence_when_dated() {
    let oldest = 1_782_302_400;
    let day = timestamps::local_day(oldest, "UTC", "en").expect("a day");
    assert_eq!(
        stats_label(&detail(Some(oldest)), "UTC", "en"),
        Some(l10n::reveal_stats_a11y(
            214,
            &day,
            &languages_text(&["en".to_owned(), "nl".to_owned()])
        ))
    );
    assert_eq!(stats_label(&detail(None), "UTC", "en"), None);
}
