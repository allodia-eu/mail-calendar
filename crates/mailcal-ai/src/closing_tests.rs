use super::without_closing;

const SIGNATURE: &str = "Met vriendelijke groet,\nSanne de Vries | Projects";

#[test]
fn a_closing_that_repeats_the_signature_is_taken_off() {
    let reply = "Hi Marc,\n\nHierbij de tekeningen.\n\nMet vriendelijke groet,\n\
                 Sanne de Vries | Projects";
    assert_eq!(
        without_closing(reply, Some(SIGNATURE)),
        "Hi Marc,\n\nHierbij de tekeningen."
    );
}

#[test]
fn a_closing_that_opens_like_the_signature_is_taken_off_whatever_name_follows() {
    let reply = "Hi Marc,\n\nHierbij de tekeningen.\n\nmet vriendelijke groet\nSanne";
    assert_eq!(
        without_closing(reply, Some(SIGNATURE)),
        "Hi Marc,\n\nHierbij de tekeningen."
    );
}

#[test]
fn a_closing_made_only_of_signature_lines_is_taken_off() {
    let reply = "Hi Marc,\n\nHierbij de tekeningen.\n\nSanne de Vries | Projects";
    assert_eq!(
        without_closing(reply, Some(SIGNATURE)),
        "Hi Marc,\n\nHierbij de tekeningen."
    );
}

#[test]
fn a_last_paragraph_that_says_something_else_stays() {
    let reply = "Hi Marc,\n\nHierbij de tekeningen.\n\nGroeten,\nSanne";
    assert_eq!(without_closing(reply, Some(SIGNATURE)), reply);
}

#[test]
fn without_a_signature_or_with_one_paragraph_nothing_is_taken() {
    let reply = "Hi Marc,\n\nMet vriendelijke groet,\nSanne";
    assert_eq!(without_closing(reply, None), reply);
    assert_eq!(without_closing(reply, Some("  ")), reply);
    let single = "Met vriendelijke groet,\nSanne";
    assert_eq!(without_closing(single, Some(SIGNATURE)), single);
}

#[test]
fn a_signature_s_delimiter_is_not_its_opening_line() {
    let reply = "Hi Marc,\n\nTot dan.\n\nSanne de Vries";
    assert_eq!(
        without_closing(reply, Some("-- \nSanne de Vries\nAllodia")),
        "Hi Marc,\n\nTot dan."
    );
}
