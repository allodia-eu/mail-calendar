use serde_json::json;

use super::{Exemplars, Habit, LanguageStyle, SCHEMA_VERSION, StyleGuide};

/// A guide written by a newer version carries fields this one does not know. Editing the notes
/// here and writing it back must keep them, or two devices on different versions erase each
/// other's work.
#[test]
fn a_guide_keeps_fields_it_does_not_know_through_an_edit() {
    let newer = json!({
        "schema_version": 2,
        "notes": "Short, please.",
        "tone_by_recipient": { "work": "formal" },
        "languages": {
            "nl": { "register": "je", "greetings": [{ "text": "Hoi", "share": 70 }],
                    "rhythm": "staccato" }
        },
        "learned": { "learned_at": 1_700_000_000, "model_family": "x" }
    });
    let mut guide: StyleGuide = serde_json::from_value(newer).unwrap();
    guide.notes = "Shorter, please.".to_owned();

    let written = serde_json::to_value(&guide).unwrap();
    assert_eq!(written["schema_version"], 2);
    assert_eq!(written["notes"], "Shorter, please.");
    assert_eq!(written["tone_by_recipient"]["work"], "formal");
    assert_eq!(written["languages"]["nl"]["rhythm"], "staccato");
    assert_eq!(written["languages"]["nl"]["greetings"][0]["text"], "Hoi");
    assert_eq!(written["learned"]["model_family"], "x");
}

#[test]
fn exemplars_keep_fields_they_do_not_know_too() {
    let newer = json!({ "schema_version": 3, "languages": { "en": ["Thanks!"] }, "weights": [1] });
    let exemplars: Exemplars = serde_json::from_value(newer).unwrap();
    let written = serde_json::to_value(&exemplars).unwrap();
    assert_eq!(written["weights"], json!([1]));
    assert_eq!(written["languages"]["en"][0], "Thanks!");
}

#[test]
fn a_new_guide_is_at_the_current_version() {
    assert_eq!(StyleGuide::new().schema_version, SCHEMA_VERSION);
    assert_eq!(Exemplars::new().schema_version, SCHEMA_VERSION);
}

/// A model can put a whole paragraph of somebody's message into a "phrase". The bound drops it
/// rather than cutting it, because a cut quotation is still a quotation.
#[test]
fn a_learned_style_is_bounded_before_it_is_kept() {
    let paragraph = "word ".repeat(40);
    let style = LanguageStyle {
        register: "x".repeat(2_000),
        phrases: vec![
            "Kind regards".to_owned(),
            paragraph.clone(),
            "  ".to_owned(),
        ],
        greetings: (0..20)
            .map(|index| Habit {
                text: format!("Hi {index}"),
                share: 250,
            })
            .collect(),
        signs_as: paragraph,
        ..LanguageStyle::default()
    }
    .bounded();

    assert_eq!(style.register.chars().count(), 600);
    assert_eq!(style.phrases, ["Kind regards"]);
    assert_eq!(style.greetings.len(), 12);
    assert!(style.greetings.iter().all(|habit| habit.share <= 100));
    assert!(style.signs_as.is_empty());
}

#[test]
fn a_headline_keeps_five_words_and_the_paragraph_count_stays_plausible() {
    let style = LanguageStyle {
        register_headline: "  Friendly professional, shifting with the recipient\n".to_owned(),
        structure_headline: "Thanks, then the answer".to_owned(),
        moves_headline: "x".repeat(200),
        typical_paragraphs: 40,
        ..LanguageStyle::default()
    }
    .bounded();

    assert_eq!(
        style.register_headline,
        "Friendly professional, shifting with the"
    );
    assert_eq!(style.structure_headline, "Thanks, then the answer");
    assert_eq!(style.moves_headline.chars().count(), 60);
    assert_eq!(style.typical_paragraphs, 12);
}

#[test]
fn debug_output_carries_no_text() {
    let mut guide = StyleGuide::new();
    guide.notes = "secret note".to_owned();
    guide.languages.insert(
        "en".to_owned(),
        LanguageStyle {
            phrases: vec!["secret phrase".to_owned()],
            ..LanguageStyle::default()
        },
    );
    let mut exemplars = Exemplars::new();
    exemplars
        .languages
        .insert("en".to_owned(), vec!["secret passage".to_owned()]);

    let printed = format!("{guide:?} {:?} {exemplars:?}", guide.languages["en"]);
    assert!(!printed.contains("secret"), "{printed}");
}

#[test]
fn the_main_language_is_the_one_most_messages_were_learned_from() {
    let mut guide = StyleGuide::new();
    guide
        .languages
        .insert("en".to_owned(), LanguageStyle::default());
    guide
        .languages
        .insert("nl".to_owned(), LanguageStyle::default());
    assert_eq!(guide.main_language(), Some("en"));
    guide.learned = Some(super::Provenance {
        messages_per_language: [("en".to_owned(), 3), ("nl".to_owned(), 30)].into(),
        ..super::Provenance::default()
    });
    assert_eq!(guide.main_language(), Some("nl"));
}
