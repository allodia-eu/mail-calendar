use super::lean;

#[test]
fn line_ends_are_one_newline_and_trailing_spaces_go() {
    assert_eq!(
        lean("Hoi Sanne,  \r\nDank je.\t\r\nGroet\u{a0}\r"),
        "Hoi Sanne,\nDank je.\nGroet\n"
    );
}

#[test]
fn runs_of_blank_lines_collapse_to_one() {
    assert_eq!(
        lean("Hoi,\n\n\n\nDank je.\n  \n \n\nGroet"),
        "Hoi,\n\nDank je.\n\nGroet"
    );
    assert_eq!(lean("Hoi,\n\nDank je."), "Hoi,\n\nDank je.");
}

#[test]
fn a_link_target_after_its_text_is_dropped() {
    assert_eq!(
        lean(
            "Zie onze site<https://example.nl/a?b=c> of mail info@example.nl \
             <mailto:info@example.nl>.\nLinkedIn <http://example.nl/in>"
        ),
        "Zie onze site of mail info@example.nl.\nLinkedIn"
    );
}

#[test]
fn a_link_with_no_text_before_it_or_not_closed_stays() {
    let text = "<https://example.nl/standalone>\nZie <https://example.nl/open en <b>vet</b>.";
    assert_eq!(lean(text), text);
    assert_eq!(lean("a <ftp://example.nl>"), "a <ftp://example.nl>");
}

#[test]
fn a_signature_as_a_plain_text_conversion_writes_it_keeps_its_words_and_little_else() {
    let signature = "Met vriendelijke groet,\r\n\r\n\r\n\r\nSanne de Vries  \r\nProjects \
                     <https://example.nl/team>\r\n\r\n\r\nexample.nl<https://example.nl>\r\n";
    assert_eq!(
        lean(signature),
        "Met vriendelijke groet,\n\nSanne de Vries\nProjects\n\nexample.nl\n"
    );
}
