//! Tests for the iTIP writer: the `METHOD:REPLY` object and the storable form of an
//! invitation.
//!
//! These are the checks that can fail *before* a message is sent, which is the whole reason the
//! writer is a pure function: the alternative is discovering that a reply was malformed by
//! noticing, days later, that an organiser never saw it.

use engine_api::{ParticipationStatus, UtcDateTime};

use super::{Reply, for_storage, reply};

fn at(year: i32, month: u8, day: u8, hour: u8, minute: u8) -> UtcDateTime {
    UtcDateTime::new(year, month, day, hour, minute, 0).expect("a valid instant")
}

fn accepted<'a>() -> Reply<'a> {
    Reply {
        uid: "040000008200E00074C5B7101A82E008",
        sequence: 1,
        organizer: "mailto:boss@example.org",
        attendee: "info@example.net",
        attendee_name: Some("Alice"),
        status: ParticipationStatus::Accepted,
        starts_at: at(2026, 8, 6, 11, 15),
        recurrence_id: None,
        stamp: at(2026, 8, 4, 12, 53),
        comment: None,
    }
}

/// Every line of the document, unfolded, so an assertion can name a property without caring
/// where the 75-octet limit happened to fall.
fn lines(document: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for physical in document.split("\r\n").filter(|line| !line.is_empty()) {
        if let Some(continuation) = physical.strip_prefix(' ')
            && let Some(last) = out.last_mut()
        {
            last.push_str(continuation);
        } else {
            out.push(physical.to_owned());
        }
    }
    out
}

#[test]
fn a_reply_carries_the_organizer_the_replier_and_nothing_else() {
    // RFC 5546 §3.2.3: a REPLY names the organiser and *only* the attendee replying. The other
    // invitees' answers are not ours to forward, and an organiser's scheduler is entitled to
    // read the attendee list it receives as authoritative: so a second ATTENDEE line here
    // would not merely be noise, it would overwrite what the organiser knows about them.
    let document = reply(&accepted());
    let lines = lines(&document);

    assert_eq!(
        lines.iter().filter(|l| l.starts_with("ATTENDEE")).count(),
        1,
    );
    assert!(lines.contains(&"ORGANIZER:mailto:boss@example.org".to_owned()));
    assert!(
        lines
            .iter()
            .any(|l| l.contains("PARTSTAT=ACCEPTED") && l.ends_with(":mailto:info@example.net")),
    );
}

#[test]
fn a_reply_is_a_reply_and_says_which_revision_it_answers() {
    let document = reply(&accepted());
    let lines = lines(&document);
    assert!(lines.contains(&"METHOD:REPLY".to_owned()));
    assert!(lines.contains(&"VERSION:2.0".to_owned()));
    assert!(lines.contains(&"UID:040000008200E00074C5B7101A82E008".to_owned()));
    // The SEQUENCE is the invitation's, not zero: it is how an organiser who has since moved
    // the meeting can tell this answer is against a revision they superseded (RFC 5546 §2.1.5).
    assert!(lines.contains(&"SEQUENCE:1".to_owned()));
    assert!(lines.contains(&"DTSTAMP:20260804T125300Z".to_owned()));
    assert!(lines.contains(&"DTSTART:20260806T111500Z".to_owned()));
}

#[test]
fn the_answer_is_spelled_the_icalendar_way_not_the_engine_way() {
    // The engine carries the JSCalendar spelling (`needs-action`, lowercase); PARTSTAT is
    // uppercase (RFC 5545 §3.2.12). Getting this backwards produces a document that parses and
    // means nothing.
    for (status, expected) in [
        (ParticipationStatus::Accepted, "PARTSTAT=ACCEPTED"),
        (ParticipationStatus::Tentative, "PARTSTAT=TENTATIVE"),
        (ParticipationStatus::Declined, "PARTSTAT=DECLINED"),
    ] {
        let document = reply(&Reply {
            status,
            ..accepted()
        });
        assert!(document.contains(expected), "missing {expected}");
    }
}

#[test]
fn replying_to_one_occurrence_names_it() {
    // Dropping the RECURRENCE-ID would answer the whole series; accepting (or worse,
    // declining) every future standup because the user answered one of them.
    let document = reply(&Reply {
        recurrence_id: Some(at(2026, 8, 6, 11, 15)),
        ..accepted()
    });
    assert!(lines(&document).contains(&"RECURRENCE-ID:20260806T111500Z".to_owned()));
}

#[test]
fn a_series_reply_names_no_occurrence() {
    assert!(!reply(&accepted()).contains("RECURRENCE-ID"));
}

#[test]
fn a_note_rides_along_and_a_blank_one_does_not() {
    let with_note = reply(&Reply {
        comment: Some("Running ten minutes late"),
        ..accepted()
    });
    assert!(lines(&with_note).contains(&"COMMENT:Running ten minutes late".to_owned()));

    // A client that always sends its (untouched) note field must not produce an empty COMMENT
    // property, which is a property whose value is missing rather than a note that is blank.
    for blank in ["", "   ", "\n"] {
        let document = reply(&Reply {
            comment: Some(blank),
            ..accepted()
        });
        assert!(
            !document.contains("COMMENT"),
            "empty note {blank:?} emitted"
        );
    }
}

#[test]
fn text_values_are_escaped_and_addresses_are_not() {
    // TEXT escaping (RFC 5545 §3.3.11) and a CAL-ADDRESS URI are different value types, and
    // applying the text rule to the address would rewrite a legal `mailto:` into one that
    // matches no attendee.
    let document = reply(&Reply {
        comment: Some("Room A, floor 3; bring the \\ slides"),
        ..accepted()
    });
    assert!(
        lines(&document).contains(&"COMMENT:Room A\\, floor 3\\; bring the \\\\ slides".to_owned())
    );
}

#[test]
fn an_address_that_already_names_its_scheme_does_not_get_a_second_one() {
    // The invitation writes `mailto:` inconsistently, and `mailto:mailto:boss@…` matches
    // nobody: the reply would be delivered and then silently ignored.
    for written in ["boss@example.org", "mailto:boss@example.org"] {
        let document = reply(&Reply {
            organizer: written,
            ..accepted()
        });
        assert!(
            lines(&document).contains(&"ORGANIZER:mailto:boss@example.org".to_owned()),
            "{written:?} produced {document}"
        );
    }
}

#[test]
fn a_display_name_that_would_break_the_line_is_quoted_or_dropped() {
    // Parameter values have no backslash escape at all (RFC 5545 §3.1): one containing `:`,
    // `;` or `,` must be quoted, and one containing a double quote cannot be represented, so
    // the quote is dropped rather than emitted, which would end the parameter early and make
    // the rest of the ATTENDEE line unparseable.
    let document = reply(&Reply {
        attendee_name: Some("Doe, Alice"),
        ..accepted()
    });
    assert!(document.contains("CN=\"Doe, Alice\""));

    let document = reply(&Reply {
        attendee_name: Some("Alice \"Ally\" Doe"),
        ..accepted()
    });
    assert!(document.contains("CN=Alice Ally Doe"), "{document}");
}

#[test]
fn a_nameless_attendee_gets_no_empty_cn() {
    for name in [None, Some(""), Some("  ")] {
        let document = reply(&Reply {
            attendee_name: name,
            ..accepted()
        });
        assert!(!document.contains("CN="), "{name:?} produced {document}");
    }
}

#[test]
fn every_physical_line_fits_the_octet_limit() {
    // RFC 5545 §3.1 folds at 75 **octets**, not characters. A catalog of Cyrillic or emoji
    // notes folded by character count produces lines two to four times the limit, which the
    // strict parsers reject: so the assertion is on `len()`, deliberately not `chars().count()`.
    let document = reply(&Reply {
        uid: &"u".repeat(400),
        comment: Some(&"жёлтая подводная лодка ".repeat(20)),
        attendee_name: Some(&"Ä".repeat(90)),
        ..accepted()
    });
    for physical in document.split("\r\n") {
        assert!(
            physical.len() <= 75,
            "{} octets: {physical:?}",
            physical.len()
        );
    }
}

#[test]
fn folding_never_splits_a_character() {
    // A parser unfolds by deleting the CRLF-and-space *before* it decodes anything, so a
    // multi-byte character cut across a fold is not recoverable; it is mojibake in the
    // organiser's inbox. `String` cannot even hold the broken form, so the check that this is
    // right is that the document round-trips to the note we put in.
    let note = "🎉".repeat(60);
    let document = reply(&Reply {
        comment: Some(&note),
        ..accepted()
    });
    let unfolded = lines(&document);
    assert!(
        unfolded
            .iter()
            .any(|line| line == &format!("COMMENT:{note}")),
        "the note did not survive folding"
    );
}

/// A Microsoft invitation as it arrives: a `METHOD`, a `VTIMEZONE` the meeting's start depends
/// on, and `X-` properties no projection models.
const INVITATION: &str = "BEGIN:VCALENDAR\r\n\
     METHOD:REQUEST\r\n\
     PRODID:Microsoft Exchange Server 2010\r\n\
     VERSION:2.0\r\n\
     BEGIN:VTIMEZONE\r\n\
     TZID:W. Europe Standard Time\r\n\
     END:VTIMEZONE\r\n\
     BEGIN:VEVENT\r\n\
     UID:meeting-1@example.org\r\n\
     SUMMARY:Sprint planning\r\n\
     X-MICROSOFT-CDO-BUSYSTATUS:BUSY\r\n\
     ORGANIZER;CN=Boss:mailto:boss@example.org\r\n\
     ATTENDEE;PARTSTAT=NEEDS-ACTION;RSVP=TRUE:mailto:info@example.net\r\n\
     END:VEVENT\r\n\
     END:VCALENDAR\r\n";

#[test]
fn storing_an_invitation_drops_the_method_and_keeps_everything_else() {
    // RFC 4791 §4.1 forbids METHOD on a stored resource; Sabre/DAV rejects the PUT outright.
    // Everything else has to survive: the VTIMEZONE the start resolves against, the X-
    // properties, and above all the ATTENDEE line, which is the only reason to store the
    // invitation rather than a plain appointment.
    let stored = for_storage(INVITATION).expect("a calendar object");
    assert!(!stored.contains("METHOD:REQUEST"));
    assert!(stored.contains("BEGIN:VTIMEZONE"));
    assert!(stored.contains("X-MICROSOFT-CDO-BUSYSTATUS:BUSY"));
    assert!(stored.contains("ATTENDEE;PARTSTAT=NEEDS-ACTION;RSVP=TRUE:mailto:info@example.net"));
    assert!(stored.contains("ORGANIZER;CN=Boss:mailto:boss@example.org"));

    // Byte-for-byte but for the one line: anything else changing here means a re-serialization
    // crept in, and a re-serialization drops what the projection does not model.
    assert_eq!(stored, INVITATION.replace("METHOD:REQUEST\r\n", ""));
}

#[test]
fn a_folded_method_loses_its_continuation_lines_too() {
    // Dropping the first physical line of a folded property leaves its continuations behind as
    // orphans, which parse as a garbage property, or, worse, as the *previous* property's
    // continuation.
    let raw = "BEGIN:VCALENDAR\r\nMETHOD:REQ\r\n UEST\r\nVERSION:2.0\r\nEND:VCALENDAR\r\n";
    let stored = for_storage(raw).expect("a calendar object");
    assert_eq!(
        stored,
        "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nEND:VCALENDAR\r\n"
    );
}

#[test]
fn a_method_nested_inside_a_component_is_left_alone() {
    // METHOD is defined only at the calendar level. Bounding the rule by depth means a
    // vendor component that invented a property of that name cannot silently lose a line.
    let raw = "BEGIN:VCALENDAR\r\nMETHOD:REQUEST\r\n\
               BEGIN:X-VENDOR\r\nMETHOD:local\r\nEND:X-VENDOR\r\nEND:VCALENDAR\r\n";
    let stored = for_storage(raw).expect("a calendar object");
    assert!(stored.contains("METHOD:local"));
    assert!(!stored.contains("METHOD:REQUEST"));
}

#[test]
fn something_that_is_not_a_calendar_object_is_not_storable() {
    assert!(for_storage("not a calendar at all").is_none());
    assert!(for_storage("").is_none());
}

#[test]
fn an_invitation_with_no_method_is_already_storable() {
    let raw = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nEND:VCALENDAR\r\n";
    assert_eq!(for_storage(raw).as_deref(), Some(raw));
}
