//! Signatures as a host reaches them: a save dispatched and returning at once, its outcome on the
//! page, a double press saving once, a failed write said by the settings notice, and the two
//! rules an editor asks before the person presses.

use std::{
    path::PathBuf,
    sync::{Arc, mpsc},
    time::{Duration, Instant},
};

use crate::{
    Intent, LogLevel, MailcalApp, NoticeKind, SignatureImageRefusal, SignatureIntent,
    SignatureOutcome, SignatureRefusal, SignatureResult, Surface, signature_image_refusal,
    signature_name_refusal,
    tests::{ChannelObserver, NullLogger, RecordingCredentialStore, RecordingStoreHandle},
};

fn app(name: &str) -> (Arc<MailcalApp>, PathBuf, mpsc::Receiver<Surface>) {
    let (tx, rx) = mpsc::channel();
    let dir = crate::tests::temp_data_dir(name);
    let app = MailcalApp::new_accounts(
        Box::new(ChannelObserver { tx }),
        Box::new(NullLogger),
        LogLevel::Info,
        Vec::new(),
        dir.to_string_lossy().into_owned(),
        "Etc/UTC".to_owned(),
        crate::analytics::test_device(),
        Box::new(RecordingStoreHandle(Arc::new(
            RecordingCredentialStore::default(),
        ))),
    )
    .expect("the app boots");
    (app, dir, rx)
}

fn save(app: &MailcalApp, id: &str, name: &str) {
    app.dispatch(Intent::Signature {
        intent: SignatureIntent::Save {
            id: id.to_owned(),
            name: name.to_owned(),
            body_html: format!("<p>{name}</p>"),
            body_plain: name.to_owned(),
        },
    });
}

/// The outcome of the change after `serial`, once there is one.
fn outcome_after(app: &MailcalApp, serial: u64) -> SignatureOutcome {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let page = app.signatures_page();
        if let Some(outcome) = page.outcome.filter(|outcome| outcome.serial > serial)
            && !page.save.running
        {
            return outcome;
        }
        assert!(Instant::now() < deadline, "the change never ended");
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// A new signature saved twice in a row, as a double press sends it, is one signature, written
/// to the library's file by the time the app closes.
#[test]
fn a_double_press_on_save_saves_one_signature() {
    let (app, dir, rx) = app("signatures-double-press");
    let id = app.signatures_page().new_id;

    save(&app, &id, "Work");
    save(&app, &id, "Work");
    let outcome = outcome_after(&app, 0);
    assert_eq!(outcome.id, id);
    assert_eq!(outcome.result, SignatureResult::Saved);
    save(&app, &id, "Work");
    outcome_after(&app, outcome.serial);

    let page = app.signatures_page();
    assert_eq!(page.signatures.len(), 1);
    assert_eq!(page.signatures[0].id, id);
    assert_ne!(page.new_id, id);
    assert!(rx.try_iter().any(|s| matches!(s, Surface::Signatures)));

    app.finish_saving_settings();
    let stored = mailcal_account::load_signatures(mailcal_account::signatures_path(&dir));
    assert_eq!(stored.ordered().len(), 1);
}

#[test]
fn a_refused_save_says_why_and_stores_nothing() {
    let (app, _dir, _rx) = app("signatures-refused");
    let id = app.signatures_page().new_id;
    save(&app, &id, "  ");
    assert_eq!(
        outcome_after(&app, 0).result,
        SignatureResult::Refused {
            reason: SignatureRefusal::BlankName
        }
    );
    assert_eq!(app.signatures_page().signatures, []);
}

/// ⚠️ A library that cannot be written is the settings notice, never silence: the signature
/// holds for this run and the notice offers Try again.
#[test]
fn a_library_that_could_not_be_written_is_the_settings_notice() {
    let (app, dir, _rx) = app("signatures-not-written");
    std::fs::create_dir_all(dir.join("signatures.toml.saving")).unwrap();
    let id = app.signatures_page().new_id;

    save(&app, &id, "Work");

    assert_eq!(outcome_after(&app, 0).result, SignatureResult::NotWritten);
    assert_eq!(app.signatures_page().signatures.len(), 1);
    assert!(
        app.notices()
            .notices
            .iter()
            .any(|notice| notice.kind == NoticeKind::SettingsNotSaved)
    );
}

#[test]
fn a_deletion_is_dispatched_and_its_row_goes() {
    let (app, _dir, _rx) = app("signatures-delete");
    let id = app.signatures_page().new_id;
    save(&app, &id, "Work");
    let saved = outcome_after(&app, 0);

    app.dispatch(Intent::Signature {
        intent: SignatureIntent::Delete { id: id.clone() },
    });

    assert_eq!(
        outcome_after(&app, saved.serial).result,
        SignatureResult::Deleted
    );
    assert_eq!(app.signatures_page().signatures, []);
}

#[test]
fn the_editors_rules_are_the_cores() {
    assert_eq!(signature_image_refusal("IMAGE/PNG".to_owned(), 1024), None);
    assert_eq!(
        signature_image_refusal("image/svg+xml".to_owned(), 1024),
        Some(SignatureImageRefusal::Format)
    );
    assert_eq!(
        signature_image_refusal("image/jpeg".to_owned(), 512 * 1024 + 1),
        Some(SignatureImageRefusal::TooLarge {
            limit_bytes: 512 * 1024
        })
    );
    assert_eq!(
        signature_name_refusal(" ".to_owned()),
        Some(SignatureRefusal::BlankName)
    );
    assert_eq!(signature_name_refusal("Work".to_owned()), None);
}
