//! The printed page: the header drawn as escaped text, the body chosen and carried over, and the
//! reading document's policy kept.

use super::{PrintHeaderLine, render_print_document};

fn line(label: &str, value: &str) -> PrintHeaderLine {
    PrintHeaderLine {
        label: label.to_owned(),
        value: value.to_owned(),
    }
}

#[test]
fn the_header_is_the_subject_then_each_line_in_the_order_given() {
    let doc = render_print_document(
        "Lunch on Friday",
        &[
            line("From", "Anna Bakker <anna@example.com>"),
            line("To", "you@example.com"),
            line("Sent", "3 July 2026 at 14:03"),
        ],
        Some("<p>Are we still on?</p>"),
        None,
        false,
    );
    let subject = doc.find("Lunch on Friday").unwrap();
    let from = doc.find("From:").unwrap();
    let to = doc.find("To:").unwrap();
    let sent = doc.find("Sent:").unwrap();
    let body = doc.find("<p>Are we still on?</p>").unwrap();
    assert!(subject < from && from < to && to < sent && sent < body);
    assert!(doc.contains("Anna Bakker &lt;anna@example.com&gt;"));
}

#[test]
fn a_line_with_nothing_to_say_is_left_out() {
    let doc = render_print_document(
        "s",
        &[line("To", "you@example.com"), line("Cc", "")],
        None,
        None,
        false,
    );
    assert!(doc.contains("To:"));
    assert!(!doc.contains("Cc:"));
}

#[test]
fn a_header_value_is_text_and_never_markup() {
    let doc = render_print_document(
        "<img src=x onerror=alert(1)>",
        &[line("From", "\"Eve\" <eve@example.com><script>")],
        None,
        None,
        false,
    );
    assert!(!doc.contains("<img"));
    assert!(!doc.contains("<script>"));
    assert!(doc.contains("&lt;img src=x onerror=alert(1)&gt;"));
    assert!(doc.contains("&quot;Eve&quot; &lt;eve@example.com&gt;&lt;script&gt;"));
}

#[test]
fn a_plain_body_is_escaped_and_keeps_its_line_breaks() {
    let doc = render_print_document("s", &[], None, Some("a < b\n\nc & d"), false);
    assert!(doc.contains("<div class=\"mailcal-print-plain\">a &lt; b\n\nc &amp; d</div>"));
    assert!(doc.contains(".mailcal-print-plain{white-space:pre-wrap}"));
}

#[test]
fn the_html_body_wins_over_the_plain_one() {
    let doc = render_print_document("s", &[], Some("<p>rich</p>"), Some("plain"), false);
    assert!(doc.contains("<p>rich</p>"));
    assert!(!doc.contains("mailcal-print-plain\">"));
}

#[test]
fn printing_loads_remote_images_only_when_reading_did() {
    // The page is the reading document with a header on top, so it inherits the CSP, and with it
    // the reader's per-message choice: a print must never be the way a tracking pixel fires.
    let blocked = render_print_document("s", &[], Some("<p>x</p>"), None, false);
    assert!(blocked.contains("default-src 'none'"));
    assert!(blocked.contains("img-src data:;"));
    let loaded = render_print_document("s", &[], Some("<p>x</p>"), None, true);
    assert!(loaded.contains("img-src https: http: data:;"));
}
