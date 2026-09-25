use super::without_closing;
use crate::Habit;

const SIGNATURE: &str = "Met vriendelijke groet,\nSanne de Vries | Projects";

fn sign_offs() -> Vec<Habit> {
    ["Groeten,", "Hartelijke groet"]
        .map(|text| Habit {
            text: text.to_owned(),
            share: 40,
        })
        .to_vec()
}

#[test]
fn a_last_line_that_is_one_of_the_person_s_sign_offs_is_taken_off_above_a_signature() {
    for last in ["Groeten", "groeten!", "  Hartelijke   groet,"] {
        let reply = format!("Hi Marc,\n\nHierbij de tekeningen.\n\n{last}");
        assert_eq!(
            without_closing(&reply, Some(SIGNATURE), &sign_offs()),
            "Hi Marc,\n\nHierbij de tekeningen.",
            "{last}"
        );
    }
}

#[test]
fn a_short_last_line_that_is_not_a_sign_off_or_has_more_to_it_stays() {
    for last in ["Tot morgen!", "Groeten aan je collega's", "Groeten,\nSanne"] {
        let reply = format!("Hi Marc,\n\nHierbij de tekeningen.\n\n{last}");
        assert_eq!(
            without_closing(&reply, Some(SIGNATURE), &sign_offs()),
            reply,
            "{last}"
        );
    }
}

#[test]
fn without_a_signature_a_sign_off_is_the_closing_and_stays() {
    let reply = "Hi Marc,\n\nHierbij de tekeningen.\n\nGroeten";
    assert_eq!(without_closing(reply, None, &sign_offs()), reply);
}

#[test]
fn a_sign_off_of_more_than_six_words_is_never_a_closing_line() {
    let long = Habit {
        text: "Laat maar weten als je vragen hebt".to_owned(),
        share: 20,
    };
    let reply = "Hi Marc,\n\nHierbij de tekeningen.\n\nLaat maar weten als je vragen hebt";
    assert_eq!(without_closing(reply, Some(SIGNATURE), &[long]), reply);
}

#[test]
fn a_closing_that_repeats_the_signature_is_taken_off() {
    let reply = "Hi Marc,\n\nHierbij de tekeningen.\n\nMet vriendelijke groet,\n\
                 Sanne de Vries | Projects";
    assert_eq!(
        without_closing(reply, Some(SIGNATURE), &[]),
        "Hi Marc,\n\nHierbij de tekeningen."
    );
}

#[test]
fn a_closing_that_opens_like_the_signature_is_taken_off_whatever_name_follows() {
    let reply = "Hi Marc,\n\nHierbij de tekeningen.\n\nmet vriendelijke groet\nSanne";
    assert_eq!(
        without_closing(reply, Some(SIGNATURE), &[]),
        "Hi Marc,\n\nHierbij de tekeningen."
    );
}

#[test]
fn a_closing_made_only_of_signature_lines_is_taken_off() {
    let reply = "Hi Marc,\n\nHierbij de tekeningen.\n\nSanne de Vries | Projects";
    assert_eq!(
        without_closing(reply, Some(SIGNATURE), &[]),
        "Hi Marc,\n\nHierbij de tekeningen."
    );
}

#[test]
fn a_last_paragraph_that_says_something_else_stays() {
    let reply = "Hi Marc,\n\nHierbij de tekeningen.\n\nGroeten,\nSanne";
    assert_eq!(without_closing(reply, Some(SIGNATURE), &[]), reply);
}

#[test]
fn without_a_signature_or_with_one_paragraph_nothing_is_taken() {
    let reply = "Hi Marc,\n\nMet vriendelijke groet,\nSanne";
    assert_eq!(without_closing(reply, None, &[]), reply);
    assert_eq!(without_closing(reply, Some("  "), &[]), reply);
    let single = "Met vriendelijke groet,\nSanne";
    assert_eq!(without_closing(single, Some(SIGNATURE), &[]), single);
}

#[test]
fn a_signature_s_delimiter_is_not_its_opening_line() {
    let reply = "Hi Marc,\n\nTot dan.\n\nSanne de Vries";
    assert_eq!(
        without_closing(reply, Some("-- \nSanne de Vries\nAllodia"), &[]),
        "Hi Marc,\n\nTot dan."
    );
}
