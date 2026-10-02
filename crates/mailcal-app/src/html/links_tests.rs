//! Addresses written as text in a message body becoming links, through the whole sanitise pass.

use super::super::sanitize;

fn clean(html: &str) -> String {
    sanitize(html).html
}

#[test]
fn an_address_in_text_becomes_a_link() {
    assert_eq!(
        clean("<p>Agenda: https://example.com/a?b=1&amp;c=2.</p>"),
        "<p>Agenda: <a href=\"https://example.com/a?b=1&amp;c=2\" rel=\"noopener noreferrer\">https://example.com/a?b=1&amp;c=2</a>.</p>"
    );
}

#[test]
fn a_www_host_is_linked_over_https_and_keeps_its_written_form() {
    assert_eq!(
        clean("<div>see www.example.com</div>"),
        "<div>see <a href=\"https://www.example.com\" rel=\"noopener noreferrer\">www.example.com</a></div>"
    );
}

#[test]
fn text_already_in_a_link_is_left_alone() {
    let out = clean(r#"<a href="https://tracker.example/r?x=1">https://example.com</a>"#);
    assert_eq!(out.matches("<a ").count(), 1, "{out}");
    assert!(out.contains("https://tracker.example/r?x=1"));

    // Including text nested deeper inside the link.
    let nested = clean(r#"<a href="https://a.example"><b>go to https://b.example</b></a>"#);
    assert_eq!(nested.matches("<a ").count(), 1, "{nested}");
}

#[test]
fn text_after_a_link_is_linked_again() {
    let out = clean(r#"<p><a href="https://a.example">a</a> then https://b.example</p>"#);
    assert!(
        out.ends_with(r#"then <a href="https://b.example" rel="noopener noreferrer">https://b.example</a></p>"#),
        "{out}"
    );
}

#[test]
fn a_style_sheet_is_never_linked() {
    let sheet = "<style>.x{background:url(https://cdn.example/a.png)} a > b {}</style>";
    let out = clean(&format!("{sheet}<p>hi</p>"));
    assert!(out.contains("url(https://cdn.example/a.png)"), "{out}");
    assert!(!out.contains("<a href=\"https://cdn.example"), "{out}");
}

#[test]
fn an_address_inside_an_attribute_is_not_linked() {
    let out = clean(r#"<p title="see https://example.com">plain</p>"#);
    assert!(!out.contains("<a "), "{out}");
}

#[test]
fn markup_written_as_text_stays_text() {
    // An address followed by escaped markup: the address is linked, the markup stays escaped.
    let out = clean("<p>https://example.com&lt;script&gt;x&lt;/script&gt;</p>");
    assert_eq!(
        out,
        "<p><a href=\"https://example.com\" rel=\"noopener noreferrer\">https://example.com</a>&lt;script&gt;x&lt;/script&gt;</p>"
    );
}

#[test]
fn a_non_breaking_space_ends_the_address_and_survives() {
    let out = clean("<p>https://example.com&nbsp;next</p>");
    assert_eq!(
        out,
        "<p><a href=\"https://example.com\" rel=\"noopener noreferrer\">https://example.com</a>&nbsp;next</p>"
    );
}

#[test]
fn a_body_without_addresses_is_unchanged() {
    let html = "<table><tr><td style=\"color:#ff0000\">R&amp;D &lt;team&gt;</td></tr></table>";
    assert_eq!(clean(html), sanitize_only(html));
}

#[test]
fn a_linked_body_reports_no_remote_image() {
    // A link does not load anything, so linking an address must not raise the remote-image prompt.
    assert!(!sanitize("<p>https://example.com/pixel.png</p>").has_remote_images);
}

#[test]
fn linking_twice_changes_nothing() {
    // A quoted original is sanitised when it is seeded and again on submit.
    let once = clean("<p>https://example.com and www.example.org</p>");
    assert_eq!(clean(&once), once);
}

fn sanitize_only(html: &str) -> String {
    super::super::sanitizer().clean(html).to_string()
}
