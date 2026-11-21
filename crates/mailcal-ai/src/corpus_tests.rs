use super::{CorpusOptions, MIN_WORDS, SentMessage, build};

const ENGLISH: &str = "Thanks for sending the figures over. I have had a look at them this \
    morning and they are mostly what we expected, although the travel costs are higher than \
    last quarter. Could you check whether the hotel in Lyon was booked twice? If so, let us ask \
    for a refund before the end of the month. Otherwise we are fine to go ahead with the plan.";

const DUTCH: &str = "Dank je voor de cijfers. Ik heb er vanochtend even naar gekeken en ze zijn \
    grotendeels wat we verwachtten, al zijn de reiskosten hoger dan het vorige kwartaal. Kun je \
    nagaan of het hotel in Lyon twee keer is geboekt? Als dat zo is, dan vragen we het geld \
    terug voor het einde van de maand. Verder kunnen we met het plan door.";

fn sent(key: &str, body: &str, sent_at: i64) -> SentMessage {
    SentMessage {
        key: key.to_owned(),
        sent_at: Some(sent_at),
        recipient: Some(format!("{key}@example.eu")),
        subject: "Re: figures".to_owned(),
        body: body.to_owned(),
        calendar: false,
    }
}

#[test]
fn the_report_counts_what_was_found_kept_and_sampled_per_language() {
    let messages = vec![
        sent("a", ENGLISH, 100),
        sent(
            "b",
            &format!("{DUTCH}\n\nOp 1 jul 2026 schreef Anna:\n> iets"),
            300,
        ),
        sent("c", "Fine by me.", 200),
        sent("d", &format!("Different start. {ENGLISH}"), 50),
    ];
    let corpus = build(messages, &CorpusOptions::default());

    let report = &corpus.report;
    assert_eq!(report.found, 4);
    assert_eq!(report.usable, 3);
    assert_eq!(report.undetected, 0);
    assert_eq!(report.oldest, Some(50));
    assert_eq!(report.newest, Some(300));
    let languages: Vec<(&str, u32)> = report
        .languages
        .iter()
        .map(|language| (language.language.as_str(), language.usable))
        .collect();
    assert_eq!(languages, [("en", 2), ("nl", 1)]);
    assert!(report.languages.iter().all(|language| language.tokens > 0));

    // What goes into a prompt is the author's own text: the quoted Dutch original is gone.
    let dutch = &corpus.languages["nl"][0];
    assert!(!dutch.text.contains("schreef Anna"));
    assert!(!dutch.text.contains("iets"));
}

#[test]
fn a_message_with_too_few_words_of_its_own_says_nothing() {
    let short = "word ".repeat(MIN_WORDS - 1);
    // Long only because of what it quotes.
    let quoting = format!("{short}\n\nOn 1 Jul 2026, Anna wrote:\n> {ENGLISH}");
    let corpus = build(vec![sent("a", &quoting, 1)], &CorpusOptions::default());
    assert_eq!(corpus.report.usable, 0);
}

#[test]
fn calendar_answers_automatic_replies_and_repeats_are_left_out() {
    let mut calendar = sent("cal", ENGLISH, 1);
    calendar.calendar = true;
    let mut away = sent("away", ENGLISH, 2);
    away.subject = "Automatic reply: figures".to_owned();
    let first = sent("first", ENGLISH, 3);
    // The same text sent again, with different line breaks and case.
    let again = sent("again", &ENGLISH.to_uppercase().replace(". ", ".\n"), 4);

    let corpus = build(
        vec![calendar, away, first, again],
        &CorpusOptions::default(),
    );

    assert_eq!(corpus.report.usable, 1);
    assert_eq!(corpus.languages["en"][0].key, "first");
}

#[test]
fn a_language_outside_the_catalog_is_counted_but_not_learned() {
    let danish = "Tak for tallene. Jeg har kigget på dem i morges, og de er stort set som \
        forventet, selvom rejseudgifterne er højere end sidste kvartal. Kan du tjekke, om hotellet \
        i Lyon blev booket to gange? Hvis det er tilfældet, beder vi om at få pengene tilbage \
        inden udgangen af måneden, ellers kører vi videre med planen som aftalt med dig.";
    let corpus = build(vec![sent("dk", danish, 1)], &CorpusOptions::default());
    assert_eq!(corpus.report.usable, 1);
    assert_eq!(corpus.report.undetected, 1);
    assert!(corpus.languages.is_empty());
}

#[test]
fn the_horizon_is_reported_as_given() {
    let options = CorpusOptions {
        signatures: Vec::new(),
        horizon: Some(42),
    };
    assert_eq!(build(Vec::new(), &options).report.horizon, Some(42));
}

#[test]
fn a_sent_message_prints_no_text() {
    let printed = format!("{:?}", sent("a", "secret words", 1));
    assert!(!printed.contains("secret"));
    assert!(!printed.contains("example.eu"));
}
