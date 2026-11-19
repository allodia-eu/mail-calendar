//! The reveal's rules, each one the four clients used to restate and disagreed on.

use super::{
    HabitFrequency, RevealLanguage, RevealPage, RevealPhrases, RevealRun, card_text,
    first_sentence_end, habit_rows, letter_lines, reveal_pages, reveal_runs,
};

fn words(text: &str) -> RevealRun {
    RevealRun {
        text: text.to_owned(),
        placeholder: false,
    }
}

fn pill(text: &str) -> RevealRun {
    RevealRun {
        text: text.to_owned(),
        placeholder: true,
    }
}

#[test]
fn a_share_is_mostly_from_half_often_from_a_fifth_and_sometimes_below() {
    assert_eq!(HabitFrequency::of(100), HabitFrequency::Mostly);
    assert_eq!(HabitFrequency::of(50), HabitFrequency::Mostly);
    assert_eq!(HabitFrequency::of(49), HabitFrequency::Often);
    assert_eq!(HabitFrequency::of(20), HabitFrequency::Often);
    assert_eq!(HabitFrequency::of(19), HabitFrequency::Sometimes);
    assert_eq!(HabitFrequency::of(0), HabitFrequency::Sometimes);
}

#[test]
fn a_bar_is_its_part_of_the_list_and_unknown_shares_split_evenly() {
    let rows = habit_rows([("Hi [Name],".to_owned(), 60), ("Hello,".to_owned(), 20)]);
    assert_eq!(rows[0].relative, 75);
    assert_eq!(rows[1].relative, 25);
    assert_eq!(rows[0].runs, [words("Hi "), pill("Name"), words(",")]);

    let unknown = habit_rows([("A".to_owned(), 0), ("B".to_owned(), 0)]);
    assert_eq!(unknown[0].relative, 50);
    assert_eq!(unknown[1].frequency, HabitFrequency::Sometimes);
}

/// The Windows copy did not end a sentence on an ellipsis, the Linux copy ended one after "e.g.",
/// and Apple and Android asked the platform's sentence breaker, which no other client has.
#[test]
fn a_first_sentence_ends_where_a_reader_would_end_it() {
    let cases = [
        ("Formal. Uses titles.", "Formal"),
        ("Formal! Uses titles.", "Formal!"),
        ("Formal? Mostly.", "Formal?"),
        ("Formal… Then warmer.", "Formal…"),
        ("Keeps to 3.5 lines. Then stops.", "Keeps to 3.5 lines"),
        (
            "Short, e.g. a line or two. Then a question.",
            "Short, e.g. a line or two",
        ),
        ("One sentence only.", "One sentence only"),
        ("No stop at all", "No stop at all"),
        ("Ends. 3 lines later.", "Ends"),
    ];
    for (description, heading) in cases {
        let (got, _) = card_text("", description).unwrap();
        assert_eq!(got, heading, "{description}");
    }
    assert_eq!(first_sentence_end("A. b"), 4, "lower case continues it");
}

#[test]
fn a_card_never_shows_a_sentence_twice_and_an_empty_one_is_left_out() {
    assert_eq!(
        card_text("", "Warm and direct. Uses first names."),
        Some(("Warm and direct".to_owned(), "Uses first names.".to_owned()))
    );
    assert_eq!(
        card_text(" Warm ", " Warm and direct. "),
        Some(("Warm".to_owned(), "Warm and direct.".to_owned()))
    );
    assert_eq!(
        card_text("", "Only this."),
        Some(("Only this".to_owned(), String::new()))
    );
    assert_eq!(card_text(" ", "  "), None);
}

#[test]
fn the_letter_is_fifteen_words_a_line_within_its_bounds() {
    let lines = letter_lines(0, 0);
    assert_eq!(lines.len(), 2, "two paragraphs when the style does not say");
    assert_eq!(lines.iter().map(Vec::len).sum::<usize>(), 4);

    let lines = letter_lines(105, 3);
    assert_eq!(lines.iter().map(Vec::len).collect::<Vec<_>>(), [3, 2, 2]);
    assert!(
        (lines[0][2] - 0.58).abs() < f64::EPSILON,
        "a paragraph ends short"
    );

    let long = letter_lines(5_000, 40);
    assert_eq!(long.len(), 6);
    assert_eq!(long.iter().map(Vec::len).sum::<usize>(), 12);

    let short = letter_lines(10, 4);
    assert_eq!(
        short.iter().map(Vec::len).sum::<usize>(),
        4,
        "a line per paragraph"
    );
}

#[test]
fn a_placeholder_is_its_own_run_and_odd_brackets_are_words() {
    assert_eq!(
        reveal_runs("Dear [Title] [Surname],"),
        [
            words("Dear "),
            pill("Title"),
            words(" "),
            pill("Surname"),
            words(",")
        ]
    );
    assert_eq!(reveal_runs("Hi [],"), [words("Hi [],")]);
    assert_eq!(reveal_runs("[[Name]"), [words("["), pill("Name")]);
    assert_eq!(reveal_runs("Best"), [words("Best")]);
    assert_eq!(reveal_runs(""), []);
}

#[test]
fn a_page_no_language_has_anything_for_is_left_out() {
    let bare = RevealLanguage {
        language: "en".to_owned(),
        letter: None,
        habits: None,
        cards: Vec::new(),
        phrases: None,
    };
    assert_eq!(
        reveal_pages(std::slice::from_ref(&bare)),
        [RevealPage::Read, RevealPage::Name]
    );
    let with_phrases = RevealLanguage {
        language: "nl".to_owned(),
        phrases: Some(RevealPhrases {
            used: vec!["Groet".to_owned()],
            avoided: Vec::new(),
        }),
        ..bare.clone()
    };
    assert_eq!(
        reveal_pages(&[bare, with_phrases]),
        [RevealPage::Read, RevealPage::Phrases, RevealPage::Name]
    );
    assert!(RevealPage::Habits.is_per_language());
    assert!(!RevealPage::Name.is_per_language());
}
