//! A draft Thunderbird or Outlook stored reopens in this app's composer as they show it.
//!
//! Each fixture under `clients/composer/tests/fixtures/interop/` is a draft one of them saved to
//! the harness mailbox, kept byte for byte (its README says what was changed and why). This test
//! opens each as a stored draft through [`App::open_composer`], over the engine's own MIME
//! reading, and holds what it opens with: the kind and the files here, and the HTML and text the
//! editor receives in `interop.json`, whose reading the editor's suite holds to what the other
//! client showed (`tests/interop.test.ts`). `MAILCAL_BLESS=1` rewrites the fixture.
//!
//! A child of [`super`], over [`super::mail_fake`]'s provider.

use std::path::PathBuf;

use engine_api::MessageIdHeader;
use serde_json::{Value, json};

use super::mail_fake::{ThreadProvider, app_and_logs, stored_draft};
use crate::{App, ComposeKind, ComposeOrigin, CompositionId, Intent, MessageRef};

struct Fixture {
    name: &'static str,
    subject: &'static str,
    /// The message the draft answers or forwards, which both clients name in `In-Reply-To` and
    /// `References` alike.
    original: Option<&'static str>,
    kind: ComposeKind,
    files: &'static [&'static str],
}

const FIXTURES: &[Fixture] = &[
    Fixture {
        name: "thunderbird-new-message",
        subject: "Test mail from Thunderbird!",
        original: None,
        kind: ComposeKind::New,
        files: &["allodia-mail-diagnostics.log"],
    },
    Fixture {
        name: "thunderbird-reply",
        subject: "Re: Duplicate Message-ID (copy B)",
        original: Some("shared-dup-msgid@example.com"),
        kind: ComposeKind::Reply,
        files: &[],
    },
    Fixture {
        name: "thunderbird-forward",
        subject: "Fwd: Harness baseline message",
        original: Some("baseline-0001@test.local"),
        kind: ComposeKind::Forward,
        files: &[],
    },
    Fixture {
        name: "thunderbird-forward-with-file",
        subject: "Fwd: Message with a text attachment",
        original: Some("attachment-0004@test.local"),
        kind: ComposeKind::Forward,
        files: &["report.csv"],
    },
    Fixture {
        name: "outlook-new-message",
        subject: "Test mail from Classic Outlook!",
        original: None,
        kind: ComposeKind::New,
        files: &["allodia-mail-diagnostics.log"],
    },
    Fixture {
        name: "outlook-reply",
        subject: "RE: Harness baseline message",
        original: Some("baseline-0001@test.local"),
        kind: ComposeKind::Reply,
        files: &[],
    },
    Fixture {
        name: "outlook-forward",
        subject: "FW: Harness baseline message",
        original: Some("baseline-0001@test.local"),
        kind: ComposeKind::Forward,
        files: &[],
    },
    Fixture {
        name: "outlook-forward-with-file",
        subject: "FW: Message with a text attachment",
        original: Some("attachment-0004@test.local"),
        kind: ComposeKind::Forward,
        files: &["report.csv"],
    },
];

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../clients/composer/tests/fixtures")
}

#[tokio::test]
async fn another_clients_draft_reopens_as_it_was_stored() {
    let mut entries = Vec::new();
    for fixture in FIXTURES {
        let opening = reopen(fixture).await;
        assert_eq!(opening.kind, fixture.kind, "{}", fixture.name);
        assert!(!opening.seeds_signature, "{}", fixture.name);
        assert_eq!(opening.subject, fixture.subject, "{}", fixture.name);
        let files: Vec<&str> = opening
            .attachments
            .iter()
            .map(|file| file.file_name.as_str())
            .collect();
        assert_eq!(files, fixture.files, "{}", fixture.name);
        entries.push(json!({
            "name": fixture.name,
            "html": opening.body_html,
            "text": opening.body_text,
        }));
    }
    let lines: Vec<String> = entries
        .iter()
        .map(|entry| serde_json::to_string(entry).expect("serialisable"))
        .collect();
    let written = format!("[\n{}\n]\n", lines.join(",\n"));
    let path = fixtures_dir().join("interop.json");
    if std::env::var_os("MAILCAL_BLESS").is_some() {
        std::fs::write(&path, written).expect("the fixture is writable");
        return;
    }
    let held: Option<Value> = std::fs::read_to_string(&path)
        .ok()
        .and_then(|held| serde_json::from_str(&held).ok());
    assert!(
        held == Some(Value::Array(entries)),
        "reopening the interop drafts no longer produces what clients/composer/tests/fixtures/\
         interop.json holds; re-run with MAILCAL_BLESS=1, then run the editor's suite (`bun test` \
         in clients/composer)"
    );
}

async fn reopen(fixture: &Fixture) -> crate::ComposeOpening {
    let key = format!("interop-{}", fixture.name);
    let mut message = stored_draft(&key);
    message.envelope.subject = Some(fixture.subject.to_owned());
    if let Some(original) = fixture.original {
        let original = MessageIdHeader::new(original).expect("a Message-ID");
        message.envelope.in_reply_to = vec![original.clone()];
        message.envelope.references = vec![original];
    }
    let source = std::fs::read(
        fixtures_dir()
            .join("interop")
            .join(format!("{}.eml", fixture.name)),
    )
    .expect("the fixture is readable");
    let (app, _logs) = app_and_logs(ThreadProvider::with(vec![message]).serving(&key, &source));
    app.dispatch(Intent::RefreshMail).await;
    let staging = std::env::temp_dir().join(format!("mailcal-interop-{}", fixture.name));
    let _ = std::fs::remove_dir_all(&staging);
    open(&app, &key, &staging.to_string_lossy()).await
}

async fn open(app: &App<ThreadProvider>, key: &str, staging: &str) -> crate::ComposeOpening {
    app.open_composer(
        CompositionId::new(key).expect("a composition id"),
        ComposeOrigin::Draft {
            message: MessageRef::from_parts("acct-1", key.to_owned()).expect("a reference"),
        },
        None,
        staging,
    )
    .await
    .unwrap_or_else(|err| panic!("{key} does not reopen: {err}"))
}
