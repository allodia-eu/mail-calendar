use super::{TextSegment, find_links, segments};

/// The addresses found in `text`, as written, each with the target it opens.
fn found(text: &str) -> Vec<(&str, String)> {
    find_links(text)
        .into_iter()
        .map(|link| (&text[link.range], link.href))
        .collect()
}

fn one(text: &str) -> (String, String) {
    let links = found(text);
    assert_eq!(
        links.len(),
        1,
        "expected one link in {text:?}, found {links:?}"
    );
    let (address, href) = links.into_iter().next().expect("one link");
    (address.to_owned(), href)
}

#[test]
fn finds_web_and_mail_addresses_in_running_text() {
    assert_eq!(
        found("Agenda: https://example.com/a?b=1&c=2#top and mailto:team@example.com today"),
        vec![
            (
                "https://example.com/a?b=1&c=2#top",
                "https://example.com/a?b=1&c=2#top".to_owned()
            ),
            (
                "mailto:team@example.com",
                "mailto:team@example.com".to_owned()
            ),
        ]
    );
}

#[test]
fn a_www_host_opens_over_https() {
    assert_eq!(
        one("Visit www.example.com/docs soon"),
        (
            "www.example.com/docs".to_owned(),
            "https://www.example.com/docs".to_owned()
        )
    );
}

#[test]
fn the_sentence_punctuation_stays_out_of_the_address() {
    for (text, address) in [
        ("See https://example.com.", "https://example.com"),
        ("Is it https://example.com?", "https://example.com"),
        ("(see https://example.com/a)", "https://example.com/a"),
        ("'https://example.com/x',", "https://example.com/x"),
        ("https://example.com/a;", "https://example.com/a"),
        ("<https://example.com/a>", "https://example.com/a"),
        ("\"https://example.com/a\"", "https://example.com/a"),
    ] {
        assert_eq!(one(text).0, address, "in {text:?}");
    }
}

#[test]
fn a_bracket_the_address_opened_is_its_own() {
    assert_eq!(
        one("https://en.wikipedia.org/wiki/Mail_(protocol) is long").0,
        "https://en.wikipedia.org/wiki/Mail_(protocol)"
    );
    assert_eq!(
        one("(https://en.wikipedia.org/wiki/Mail_(protocol))").0,
        "https://en.wikipedia.org/wiki/Mail_(protocol)"
    );
}

#[test]
fn the_prefix_is_matched_in_any_case() {
    assert_eq!(one("HTTPS://EXAMPLE.COM").1, "HTTPS://EXAMPLE.COM");
    assert_eq!(one("WWW.Example.com").1, "https://WWW.Example.com");
}

#[test]
fn only_an_address_that_starts_a_word_is_one() {
    for text in [
        "xhttps://example.com",
        "foo.www.example.com",
        "user@www.example.com",
        "/www.example.com",
    ] {
        assert!(found(text).is_empty(), "expected nothing in {text:?}");
    }
}

#[test]
fn a_prefix_alone_is_not_an_address() {
    for text in [
        "https://",
        "http:// example.com",
        "https://.",
        "mailto:",
        "mailto:nobody",
        "www.",
        "www.example",
        "the www. prefix",
        "https:///path",
    ] {
        assert!(found(text).is_empty(), "expected nothing in {text:?}");
    }
}

#[test]
fn bare_domains_and_addresses_and_other_schemes_are_left_alone() {
    for text in [
        "example.com",
        "someone@example.com",
        "javascript:alert(1)",
        "ftp://example.com",
        "file:///etc/passwd",
        "data:text/html,x",
    ] {
        assert!(found(text).is_empty(), "expected nothing in {text:?}");
    }
}

#[test]
fn an_address_ends_at_whitespace_and_markup() {
    assert_eq!(
        one("https://example.com/a\u{a0}next").0,
        "https://example.com/a"
    );
    assert_eq!(
        one("https://example.com/a\nnext").0,
        "https://example.com/a"
    );
    assert_eq!(one("https://example.com/a<b>").0, "https://example.com/a");
}

#[test]
fn non_ascii_text_around_and_inside_an_address_is_byte_safe() {
    assert_eq!(
        one("Café → https://exämple.com/ü, merci").0,
        "https://exämple.com/ü"
    );
    assert!(found("ééé hhh mmm www").is_empty());
}

#[test]
fn several_addresses_are_all_found_in_order() {
    let links = found("https://a.example https://b.example\nwww.c.example");
    let addresses: Vec<&str> = links.iter().map(|(address, _)| *address).collect();
    assert_eq!(
        addresses,
        vec!["https://a.example", "https://b.example", "www.c.example"]
    );
}

#[test]
fn segments_cover_the_whole_text() {
    let text = "Join https://meet.example/abc. Thanks";
    let parts = segments(text);
    assert_eq!(
        parts,
        vec![
            TextSegment {
                text: "Join ".to_owned(),
                link: None
            },
            TextSegment {
                text: "https://meet.example/abc".to_owned(),
                link: Some("https://meet.example/abc".to_owned())
            },
            TextSegment {
                text: ". Thanks".to_owned(),
                link: None
            },
        ]
    );
    let rejoined: String = parts.iter().map(|part| part.text.as_str()).collect();
    assert_eq!(rejoined, text);
}

#[test]
fn text_without_an_address_is_one_segment_and_empty_text_is_none() {
    assert_eq!(
        segments("no links here"),
        vec![TextSegment {
            text: "no links here".to_owned(),
            link: None
        }]
    );
    assert!(segments("").is_empty());
}
