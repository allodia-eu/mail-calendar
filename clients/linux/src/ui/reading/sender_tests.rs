//! The reading header's sender line: the loaded message's full header, else the list row's.

use mailcal_bindings::ReadingSnapshot;

use super::sender_line;
use crate::ui::model::OpenedMessage;

#[test]
fn loaded_sender_uses_the_full_reading_header_and_loading_falls_back_to_the_row() {
    let opened = OpenedMessage {
        account: "account".to_owned(),
        key: "message".to_owned(),
        subject: "Subject".to_owned(),
        from: "Sender".to_owned(),
        date: "2026-07-20".to_owned(),
        avatar: crate::ui::avatar::AvatarData::from(&crate::ui::model::blank_avatar()),
    };
    let mut reading = ReadingSnapshot {
        importance_banner: None,
        avatar: crate::ui::model::blank_avatar(),
        key: "message".to_owned(),
        subject: String::new(),
        from: "Sender <sender@example.test>".to_owned(),
        to: String::new(),
        cc: String::new(),
        bcc: String::new(),
        html: None,
        plain: None,
        has_remote_images: false,
        load_error: false,
        attachments: Vec::new(),
        invitation: None,
        pending: false,
        from_name: "Sender".to_owned(),
        from_address: "sender@example.test".to_owned(),
        to_people: Vec::new(),
        cc_people: Vec::new(),
        bcc_people: Vec::new(),
        default_reply_all: false,
    };

    assert_eq!(
        sender_line(&reading, &opened),
        "Sender <sender@example.test>"
    );
    reading.from.clear();
    assert_eq!(sender_line(&reading, &opened), "Sender");
}
