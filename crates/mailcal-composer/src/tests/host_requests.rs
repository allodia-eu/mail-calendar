use crate::{
    HostAnswer, HostRequest, HostRequestKind, LinkAnswer, LinkRequest, LinkUrl, answer_script,
    link_address, parse_host_request,
};

fn link(text: &str, address: &str, removable: bool) -> HostRequest {
    HostRequest {
        id: 7,
        kind: HostRequestKind::Link(LinkRequest {
            text: text.to_owned(),
            address: address.to_owned(),
            removable,
        }),
    }
}

#[test]
fn reads_a_link_request() {
    let message = r#"{"id":7,"request":{"link":{"text":"the docs","address":"https://example.com","removable":true}}}"#;
    assert_eq!(
        parse_host_request(message),
        Some(link("the docs", "https://example.com", true))
    );
}

#[test]
fn refuses_anything_outside_the_vocabulary() {
    for message in [
        "",
        "null",
        "{}",
        r#"{"id":1}"#,
        r#"{"id":1,"request":{"open_url":{"url":"https://example.com"}}}"#,
        r#"{"id":1,"request":{"link":{"text":"","address":"","removable":false,"run":"x"}}}"#,
        r#"{"id":1,"request":{"link":{"text":"","address":""}}}"#,
        r#"{"id":-1,"request":{"link":{"text":"","address":"","removable":false}}}"#,
        r#"{"id":1,"request":{"link":{"text":"","address":"","removable":false}},"extra":1}"#,
    ] {
        assert_eq!(
            parse_host_request(message),
            None,
            "expected {message:?} to be refused"
        );
    }
}

#[test]
fn refuses_an_oversized_request() {
    let words = "a".repeat(64 * 1024);
    let message = format!(
        r#"{{"id":1,"request":{{"link":{{"text":"{words}","address":"","removable":false}}}}}}"#
    );
    assert_eq!(parse_host_request(&message), None);
}

#[test]
fn a_request_never_prints_its_words() {
    let printed = format!(
        "{:?}",
        link("secret words", "https://secret.example", false)
    );
    assert!(!printed.contains("secret"), "{printed}");
}

#[test]
fn answers_as_a_call_into_the_editor() {
    let address = LinkUrl::new("https://example.com/a").expect("valid");
    let apply = HostAnswer::Link(LinkAnswer::Apply {
        text: "the \"docs\"".to_owned(),
        address,
    });
    assert_eq!(
        answer_script(3, &apply),
        r#"window.answerComposerRequest(3, {"link":{"apply":{"text":"the \"docs\"","address":"https://example.com/a"}}});"#
    );
    assert_eq!(
        answer_script(4, &HostAnswer::Link(LinkAnswer::Remove)),
        r#"window.answerComposerRequest(4, {"link":"remove"});"#
    );
    assert_eq!(
        answer_script(5, &HostAnswer::Link(LinkAnswer::Cancel)),
        r#"window.answerComposerRequest(5, {"link":"cancel"});"#
    );
}

#[test]
fn an_answer_script_cannot_be_broken_out_of() {
    let address = LinkUrl::new("https://example.com").expect("valid");
    let text = "\");alert(1);//\u{2028}</script>".to_owned();
    let script = answer_script(1, &HostAnswer::Link(LinkAnswer::Apply { text, address }));
    assert!(
        script.contains("\\\");alert(1);//\\u2028</script>"),
        "{script}"
    );
    assert!(!script.contains('\u{2028}'));
}

#[test]
fn keeps_an_allowed_scheme_and_completes_a_host_or_an_email_address() {
    for (typed, expected) in [
        (" https://example.com/a ", "https://example.com/a"),
        ("example.com/docs", "https://example.com/docs"),
        ("example.com:8080/x", "https://example.com:8080/x"),
        ("//example.com", "https://example.com"),
        ("someone@example.com", "mailto:someone@example.com"),
        ("mailto:someone@example.com", "mailto:someone@example.com"),
        ("HTTP://Example.com", "HTTP://Example.com"),
        ("bücher.example/a", "https://bücher.example/a"),
    ] {
        assert_eq!(
            link_address(typed).as_ref().map(LinkUrl::as_str),
            Some(expected),
            "{typed:?}"
        );
    }
}

#[test]
fn refuses_what_cannot_be_a_link() {
    for typed in [
        "",
        "   ",
        "javascript:alert(1)",
        "file:///etc/passwd",
        "data:text/html,x",
        "hello",
        "exa mple.com",
        ".example.com",
        "example.",
        "https://",
        "someone@",
    ] {
        assert!(
            link_address(typed).is_none(),
            "expected {typed:?} to be refused"
        );
    }
}
