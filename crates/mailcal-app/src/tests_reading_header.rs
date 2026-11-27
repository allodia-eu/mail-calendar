//! The reading header's people: who sent an open message and who received it, as the joined
//! strings a desktop header and a printout draw and as names and addresses apart for a header
//! that names people and keeps their addresses for its details.

use std::sync::{Arc, Mutex};

use engine_api::EmailAddress;
use fakes::{FakeProvider, account, app, message, msg};
use mailcal_viewmodel::MailPerson;

use crate::{Intent, ReaderId, tests_fakes as fakes};

#[tokio::test]
async fn open_message_surfaces_to_cc_and_bcc_recipients_formatted() {
    // A stored message carrying To/Cc/Bcc; like the sender's own Sent copy, whose APPENDed
    // bytes keep the Bcc header. Opening it surfaces every recipient in the reading snapshot,
    // so the sender can see whom they Bcc'd.
    let mut m = message("m1", "a", "Quarterly report");
    m.envelope.from = vec![EmailAddress::named("Alice Smith", "alice@remote.test")];
    m.envelope.to = vec![
        EmailAddress::named("Bob Jones", "bob@remote.test"),
        EmailAddress::new("team@remote.test"),
    ];
    m.envelope.cc = vec![EmailAddress::new("carol@remote.test")];
    m.envelope.bcc = vec![EmailAddress::new("dave@remote.test")];

    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(
        vec![account("acct-1", FakeProvider::with(vec![m]))],
        &surfaces,
    );
    app.dispatch(Intent::RefreshMail).await;
    app.dispatch(Intent::OpenMessage {
        reader: ReaderId::Pane,
        message: msg("acct-1", "m1"),
    })
    .await;

    let reading = app.reading_view();
    // Named addresses render as "Name <email>", bare ones as the address; comma-joined.
    assert_eq!(reading.to, "Bob Jones <bob@remote.test>, team@remote.test");
    assert_eq!(reading.cc, "carol@remote.test");
    assert_eq!(reading.bcc, "dave@remote.test");
    // And apart, for a header that names people and keeps their addresses for the details: no
    // client splits the strings above.
    let people = |list: &[MailPerson]| -> Vec<(String, String)> {
        list.iter()
            .map(|p| (p.name.clone(), p.address.clone()))
            .collect()
    };
    assert_eq!(
        people(&reading.to_people),
        vec![
            ("Bob Jones".to_owned(), "bob@remote.test".to_owned()),
            (String::new(), "team@remote.test".to_owned()),
        ]
    );
    assert_eq!(
        people(&reading.cc_people),
        vec![(String::new(), "carol@remote.test".to_owned())]
    );
    assert_eq!(
        people(&reading.bcc_people),
        vec![(String::new(), "dave@remote.test".to_owned())]
    );
    assert_eq!(
        (
            reading.sender.name.as_str(),
            reading.sender.address.as_str()
        ),
        ("Alice Smith", "alice@remote.test")
    );
}
