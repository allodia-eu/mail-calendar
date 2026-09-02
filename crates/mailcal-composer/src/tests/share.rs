use crate::{ShareRejection, ShareRequest, SharedItem, prefill_from_share};

fn file(path: &str, name: &str, media_type: &str) -> SharedItem {
    SharedItem {
        path: path.to_owned(),
        suggested_name: name.to_owned(),
        declared_media_type: media_type.to_owned(),
    }
}

fn share(items: Vec<SharedItem>, text: &str, subject: &str) -> ShareRequest {
    ShareRequest {
        items,
        text: text.to_owned(),
        subject: subject.to_owned(),
    }
}

#[test]
fn one_shared_file_becomes_one_attachment() {
    let prefill = prefill_from_share(share(
        vec![file("/cache/stage-1", "report.pdf", "application/pdf")],
        "",
        "",
    ));

    assert_eq!(prefill.attachments.len(), 1);
    assert_eq!(prefill.attachments[0].path, "/cache/stage-1");
    assert_eq!(prefill.attachments[0].file_name, "report.pdf");
    assert_eq!(prefill.attachments[0].media_type, "application/pdf");
    assert!(prefill.rejected.is_empty());
    assert!(!prefill.is_empty());
}

#[test]
fn an_empty_share_is_empty_rather_than_an_error() {
    // A launch carrying nothing must be answerable without an error the user cannot act on;
    // a client reads `is_empty` and ignores the launch.
    let prefill = prefill_from_share(ShareRequest::default());
    assert!(prefill.is_empty());
    assert!(prefill.rejected.is_empty());
}

#[test]
fn a_share_never_carries_recipients_of_its_own() {
    // The rule that keeps a sharing app from addressing a message: text that is not a mail link
    // reaches the body and nothing else.
    let prefill = prefill_from_share(share(Vec::new(), "ada@example.test", "Hello"));
    assert_eq!(prefill.to, "");
    assert_eq!(prefill.cc, "");
    assert_eq!(prefill.bcc, "");
    assert_eq!(prefill.body, "ada@example.test");
    assert_eq!(prefill.subject, "Hello");
}

#[test]
fn shared_text_that_is_a_mail_link_goes_through_the_mailto_allowlist() {
    // The one route from a share to the recipient fields, and it is the *same* decode a tapped
    // link gets, so `from` is dropped here exactly as it is there.
    let prefill = prefill_from_share(share(
        Vec::new(),
        "mailto:ada@example.test?subject=Lunch&from=spoof@evil.test",
        "Shared from a browser",
    ));

    assert_eq!(prefill.to, "ada@example.test");
    // The link's own subject is the more specific request, so it wins over the share's.
    assert_eq!(prefill.subject, "Lunch");
    assert_eq!(prefill.body, "");
}

#[test]
fn a_mail_links_missing_subject_falls_back_to_the_shares_own() {
    let prefill = prefill_from_share(share(Vec::new(), "mailto:ada@example.test", "Quarterly"));
    assert_eq!(prefill.to, "ada@example.test");
    assert_eq!(prefill.subject, "Quarterly");
}

#[test]
fn a_shared_url_becomes_the_body() {
    let prefill = prefill_from_share(share(
        Vec::new(),
        "https://allodia.eu/mail-calendar",
        "Allodia Mail & Calendar",
    ));
    assert_eq!(prefill.body, "https://allodia.eu/mail-calendar");
    assert_eq!(prefill.subject, "Allodia Mail & Calendar");
    assert_eq!(prefill.to, "");
}

#[test]
fn a_shared_subject_cannot_inject_a_header() {
    let prefill = prefill_from_share(share(Vec::new(), "", "Lunch\r\nBcc: snoop@evil.test"));
    assert_eq!(prefill.subject, "LunchBcc: snoop@evil.test");
}

#[test]
fn shared_text_keeps_its_line_breaks_but_loses_other_controls() {
    let prefill = prefill_from_share(share(Vec::new(), "one\r\ntwo\u{0}\nthree\t.", ""));
    assert_eq!(prefill.body, "one\ntwo\nthree\t.");
}

#[test]
fn a_hostile_file_name_is_normalised_before_it_reaches_the_composer() {
    // The share path inherits the same gate the picker uses; asserted here as well because this
    // is the entry point another *application* controls, rather than the user.
    let prefill = prefill_from_share(share(
        vec![file(
            "/cache/stage-1",
            "../../etc/holiday\u{202E}gpj.exe",
            "*/*",
        )],
        "",
        "",
    ));

    assert_eq!(prefill.attachments[0].file_name, "holidaygpj.exe");
    assert_eq!(
        prefill.attachments[0].media_type,
        "application/octet-stream"
    );
}

#[test]
fn a_declared_wildcard_falls_back_to_what_the_name_implies() {
    let prefill = prefill_from_share(share(
        vec![file("/cache/stage-1", "photo.png", "*/*")],
        "",
        "",
    ));
    assert_eq!(prefill.attachments[0].media_type, "image/png");
}

#[test]
fn an_item_without_a_path_is_reported_rather_than_dropped() {
    // A file the user watched go into a share sheet and never saw again is one they will assume
    // was attached, so every refusal comes back with a reason to show.
    let prefill = prefill_from_share(share(
        vec![
            file("", "ghost.pdf", "application/pdf"),
            file("/cache/stage-1", "real.pdf", "application/pdf"),
        ],
        "",
        "",
    ));

    assert_eq!(prefill.attachments.len(), 1);
    assert_eq!(prefill.attachments[0].file_name, "real.pdf");
    assert_eq!(prefill.rejected.len(), 1);
    assert_eq!(prefill.rejected[0].name, "ghost.pdf");
    assert_eq!(prefill.rejected[0].reason, ShareRejection::NoPath);
}

#[test]
fn the_same_path_twice_is_attached_once() {
    let prefill = prefill_from_share(share(
        vec![
            file("/cache/stage-1", "report.pdf", "application/pdf"),
            file("/cache/stage-1", "report.pdf", "application/pdf"),
        ],
        "",
        "",
    ));

    assert_eq!(prefill.attachments.len(), 1);
    assert_eq!(prefill.rejected.len(), 1);
    assert_eq!(prefill.rejected[0].reason, ShareRejection::Duplicate);
}

#[test]
fn a_share_over_the_cap_keeps_the_first_items_and_reports_the_rest() {
    // The user's selection order is their own, so the cap takes from the end.
    let items = (0..25)
        .map(|index| file(&format!("/cache/stage-{index}"), "f.txt", "text/plain"))
        .collect();
    let prefill = prefill_from_share(share(items, "", ""));

    assert_eq!(prefill.attachments.len(), 20);
    assert_eq!(prefill.attachments[0].path, "/cache/stage-0");
    assert_eq!(prefill.attachments[19].path, "/cache/stage-19");
    assert_eq!(prefill.rejected.len(), 5);
    assert!(
        prefill
            .rejected
            .iter()
            .all(|item| item.reason == ShareRejection::TooMany)
    );
}

#[test]
fn a_share_of_only_an_unusable_file_is_empty_and_still_reports_why() {
    // `is_empty` answers "is there anything to open a composer with", which is a different
    // question from "was anything refused". A client needs both answers.
    let prefill = prefill_from_share(share(vec![file("", "ghost.pdf", "")], "", ""));
    assert!(prefill.is_empty());
    assert_eq!(prefill.rejected.len(), 1);
}

#[test]
fn attachments_keep_the_order_the_user_selected_them_in() {
    let prefill = prefill_from_share(share(
        vec![
            file("/cache/a", "a.pdf", "application/pdf"),
            file("/cache/b", "b.pdf", "application/pdf"),
            file("/cache/c", "c.pdf", "application/pdf"),
        ],
        "",
        "",
    ));

    let names: Vec<_> = prefill
        .attachments
        .iter()
        .map(|a| a.file_name.as_str())
        .collect();
    assert_eq!(names, ["a.pdf", "b.pdf", "c.pdf"]);
}

#[test]
fn a_share_carrying_files_and_text_seeds_both() {
    let prefill = prefill_from_share(share(
        vec![file("/cache/stage-1", "minutes.pdf", "application/pdf")],
        "Notes from today.",
        "Board meeting",
    ));

    assert_eq!(prefill.attachments.len(), 1);
    assert_eq!(prefill.body, "Notes from today.");
    assert_eq!(prefill.subject, "Board meeting");
}

#[test]
fn the_debug_impls_carry_no_content() {
    // Every one of these fields is the user's mail: names of their files, their text, who they
    // are writing to. A log line may say how much, never what (docs/logging.md).
    let request = share(
        vec![file("/cache/stage-1", "salary.pdf", "application/pdf")],
        "mailto:ada@example.test?body=confidential",
        "Salaries",
    );
    let rendered = format!("{request:?}");
    assert!(!rendered.contains("salary"), "{rendered}");
    assert!(!rendered.contains("ada@example.test"), "{rendered}");

    let prefill = format!("{:?}", prefill_from_share(request));
    assert!(!prefill.contains("salary"), "{prefill}");
    assert!(!prefill.contains("ada@example.test"), "{prefill}");
    assert!(!prefill.contains("confidential"), "{prefill}");
}
