use super::*;

#[test]
fn renders_ordered_list_with_numbered_html_and_plain_text() {
    let document = ComposerDocument {
        blocks: vec![Block::List(List {
            kind: ListKind::Ordered,
            items: vec![item(vec![text("first")]), item(vec![text("second")])],
        })],
        attachments: Vec::new(),
    };

    let output = render(&document).expect("valid document");

    assert_eq!(
        output.html,
        html_doc("<ol><li>first</li><li>second</li></ol>")
    );
    assert_eq!(output.plain_text, "1. first\n2. second");
}

#[test]
fn renders_bullet_sublist_nested_inside_an_ordered_item() {
    let document = ComposerDocument {
        blocks: vec![Block::List(List {
            kind: ListKind::Ordered,
            items: vec![
                ListItem {
                    content: vec![text("fruits")],
                    child: Some(List {
                        kind: ListKind::Bullet,
                        items: vec![item(vec![text("apple")]), item(vec![text("pear")])],
                    }),
                },
                item(vec![text("veggies")]),
            ],
        })],
        attachments: Vec::new(),
    };

    let output = render(&document).expect("valid document");

    // The sub-list lives INSIDE its parent <li>, ordered numbering skips the nested items.
    assert_eq!(
        output.html,
        html_doc("<ol><li>fruits<ul><li>apple</li><li>pear</li></ul></li><li>veggies</li></ol>")
    );
    assert_eq!(
        output.plain_text,
        "1. fruits\n  - apple\n  - pear\n2. veggies"
    );
}

#[test]
fn renders_ordered_sublist_nested_inside_a_bullet_item() {
    let document = ComposerDocument {
        blocks: vec![Block::List(List {
            kind: ListKind::Bullet,
            items: vec![ListItem {
                content: vec![text("steps")],
                child: Some(List {
                    kind: ListKind::Ordered,
                    items: vec![item(vec![text("a")]), item(vec![text("b")])],
                }),
            }],
        })],
        attachments: Vec::new(),
    };

    let output = render(&document).expect("valid document");

    assert_eq!(
        output.html,
        html_doc("<ul><li>steps<ol><li>a</li><li>b</li></ol></li></ul>")
    );
    assert_eq!(output.plain_text, "- steps\n  1. a\n  2. b");
}

#[test]
fn nested_list_schema_round_trips_through_json() {
    let document = ComposerDocument {
        blocks: vec![Block::List(List {
            kind: ListKind::Ordered,
            items: vec![ListItem {
                content: vec![text("outer")],
                child: Some(List {
                    kind: ListKind::Bullet,
                    items: vec![item(vec![text("inner")])],
                }),
            }],
        })],
        attachments: Vec::new(),
    };

    let json = serde_json::to_string(&document).expect("serialize");
    let restored: ComposerDocument = serde_json::from_str(&json).expect("deserialize");

    assert_eq!(restored, document);
}

#[test]
fn validates_inline_image_carried_inside_a_nested_list_item() {
    // The CID image lives two levels deep; if validation did not recurse, the inline
    // attachment would be reported as unused and rendering would fail.
    let document = ComposerDocument {
        blocks: vec![Block::List(List {
            kind: ListKind::Ordered,
            items: vec![ListItem {
                content: vec![text("see")],
                child: Some(List {
                    kind: ListKind::Bullet,
                    items: vec![item(vec![InlineContent::Image(InlineImage {
                        attachment_id: aid("img-1"),
                        alt_text: "chart".to_owned(),
                        width_px: None,
                    })])],
                }),
            }],
        })],
        attachments: vec![inline_attachment("img-1")],
    };

    let output = render(&document).expect("valid document");

    assert_eq!(output.inline_attachments.len(), 1);
    assert_eq!(
        output.inline_attachments[0].cid.as_ref(),
        Some(&cid("chart-1@example.test"))
    );
    assert!(output.html.contains(
        "<ol><li>see<ul><li><img src=\"cid:chart-1@example.test\" alt=\"chart\"></li></ul></li></ol>"
    ));
}

#[test]
fn rejects_missing_inline_image_inside_a_nested_list_item() {
    let document = ComposerDocument {
        blocks: vec![Block::List(List {
            kind: ListKind::Bullet,
            items: vec![ListItem {
                content: vec![text("outer")],
                child: Some(List {
                    kind: ListKind::Bullet,
                    items: vec![item(vec![InlineContent::Image(InlineImage {
                        attachment_id: aid("missing"),
                        alt_text: String::new(),
                        width_px: None,
                    })])],
                }),
            }],
        })],
        attachments: Vec::new(),
    };

    assert!(matches!(
        render(&document),
        Err(ComposerError::MissingInlineAttachment { .. })
    ));
}
