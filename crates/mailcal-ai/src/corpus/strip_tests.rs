use super::Stripper;

fn own(body: &str) -> String {
    Stripper::new().own_text(body, &[])
}

const OWN: &str = "Thanks, that works for me.\n\nSee you on Friday.";

#[test]
fn the_signature_delimiter_ends_the_author_s_text() {
    assert_eq!(own(&format!("{OWN}\n-- \nAnna Bakker\nAllodia")), OWN);
    // A client that trims trailing space leaves a bare `--`.
    assert_eq!(own(&format!("{OWN}\n--\nAnna Bakker")), OWN);
}

/// Every catalog locale's attribution, filled the way this app's clients fill it.
#[test]
fn an_attribution_in_any_catalog_locale_ends_it() {
    for line in [
        "On 3 Jul 2026 at 14:03, Anna Bakker <anna@example.eu> wrote:",
        "Op 3 jul 2026 om 14:03 schreef Anna Bakker <anna@example.eu>:",
        "Am 03.07.2026 um 14:03 schrieb Anna Bakker <anna@example.eu>:",
        // The no-break space Gmail puts before the colon.
        "Le 3 juil. 2026 à 14:03, Anna Bakker <anna@example.eu> a écrit\u{a0}:",
        "El 3 jul 2026, a las 14:03, Anna Bakker <anna@example.eu> escribió:",
        "Il giorno 3 lug 2026 alle 14:03, Anna Bakker <anna@example.eu> ha scritto:",
        "A 3 de jul. de 2026, às 14:03, Anna Bakker <anna@example.eu> escreveu:",
    ] {
        let body = format!("{OWN}\n\n{line}\n> Are we still on for Friday?");
        assert_eq!(own(&body), OWN, "{line}");
    }
}

#[test]
fn a_foreign_client_s_attribution_ends_it_too() {
    let body = format!(
        "{OWN}\n\nOp 3 jul. 2026 om 14:03 heeft Anna Bakker <anna@example.eu> het volgende \
         geschreven:\n\n> Vrijdag?"
    );
    assert_eq!(own(&body), OWN);
}

#[test]
fn an_attribution_wrapped_over_two_lines_ends_it() {
    let body = format!(
        "{OWN}\n\nOn Fri, 3 Jul 2026 at 14:03, Anna Bakker with a long name <\nanna@example.eu> \
         wrote:\n> Friday?"
    );
    assert_eq!(own(&body), OWN);
}

/// Prose that opens like an attribution but carries no date is the author's own sentence.
#[test]
fn a_sentence_shaped_like_an_attribution_is_kept() {
    let body = "Op maandag schreef ik je al:\nhet kan niet eerder.";
    assert_eq!(own(body), body);
}

#[test]
fn a_forward_ends_it_whether_labelled_by_this_app_or_framed_by_another() {
    for marker in [
        "Forwarded message",
        "Doorgestuurd bericht",
        "---------- Forwarded message ---------",
        "-----Original Message-----",
        "-----Oorspronkelijk bericht-----",
    ] {
        let body = format!("{OWN}\n\n{marker}\nFrom: Anna\nThe original.");
        assert_eq!(own(&body), OWN, "{marker}");
    }
}

#[test]
fn an_outlook_header_block_ends_it_with_or_without_its_rule() {
    let block = "From: Anna Bakker <anna@example.eu>\nSent: Friday, 3 July 2026 14:03\n\
                 To: Me\nSubject: Friday\n\nThe original.";
    assert_eq!(own(&format!("{OWN}\n\n{block}")), OWN);
    assert_eq!(
        own(&format!(
            "{OWN}\n\n________________________________\n{block}"
        )),
        OWN
    );
    let dutch =
        "Van: Anna Bakker\nVerzonden: vrijdag 3 juli 2026 14:03\nAan: mij\n\nHet origineel.";
    assert_eq!(own(&format!("{OWN}\n\n{dutch}")), OWN);
}

/// A single `From:` line is something the author may well write ("From: the finance team, a
/// question"); only a block of headers is a quoted original.
#[test]
fn a_lone_header_shaped_line_is_kept() {
    let body = "From: the finance team, a question.\nCan you send the invoice?";
    assert_eq!(own(body), body);
}

#[test]
fn three_quoted_lines_end_it_and_a_lone_one_is_dropped() {
    let body = "You asked:\n> can we move it?\nYes, Friday works.\n\n> line one\n> line two\n> \
                line three\nstill theirs";
    assert_eq!(own(body), "You asked:\nYes, Friday works.");
}

/// This app writes the signature before the quote, and a reply to a message this app sent
/// quotes that message's signature too; the first cut takes both.
#[test]
fn a_reply_to_this_app_s_own_mail_loses_both_signatures() {
    let body = format!(
        "{OWN}\n-- \nMe\n\nOn 3 Jul 2026 at 14:03, Anna <anna@example.eu> wrote:\n> Hi\n> -- \n> \
         Anna"
    );
    assert_eq!(own(&body), OWN);
}

#[test]
fn a_trailing_copy_of_the_account_s_signature_is_removed() {
    let signature = "Anna Bakker\nAllodia · Utrecht".to_owned();
    let body = format!("{OWN}\n\nAnna Bakker\nAllodia · Utrecht\n");
    assert_eq!(
        Stripper::new().own_text(&body, std::slice::from_ref(&signature)),
        OWN
    );
    // Only at the end: the same lines mid-message are the author's text.
    let middle = format!("Anna Bakker\nAllodia · Utrecht\n\n{OWN}");
    assert_eq!(Stripper::new().own_text(&middle, &[signature]), middle);
}

#[test]
fn runs_of_blank_lines_collapse() {
    assert_eq!(own("One.\n\n\n\nTwo.\n\n"), "One.\n\nTwo.");
}
