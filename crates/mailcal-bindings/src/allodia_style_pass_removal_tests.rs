//! The rest of the style pass: forgetting in both directions, the service's cap, the sign-ins that
//! do not include styles, and settling a style changed on both sides. Fixtures are the parent's.

use super::*;
use crate::MailcalError;

fn tombstone() -> &'static str {
    r#"{"id":"rec-7","version":3,"deletedAt":"2026-09-23T10:00:00Z"}"#
}

/// A signed-in grant carrying `scopes`, as the store keeps it.
fn grant(scopes: &[&str]) -> String {
    let scopes: Vec<String> = scopes.iter().map(|scope| format!("\"{scope}\"")).collect();
    format!(
        "[allodia]\nemail = \"sam@example.eu\"\nrefresh_token = \"refresh\"\ngranted_scopes = [{}]\n",
        scopes.join(", ")
    )
}

fn resolve(
    app: &MailcalApp,
    transport: &Scripted,
    book: &SyncBookkeeping,
    keep_this_device: bool,
) -> Result<(), MailcalError> {
    let service = AccountService::new("https://mailcal.example.com");
    let pass = Pass {
        service: &service,
        transport,
        token: "an-access-token",
        bookkeeping: book,
    };
    app.resolve_style_conflict(&pass, "s1", keep_this_device)
}

#[test]
fn a_style_forgotten_here_is_forgotten_at_the_service_at_the_version_it_read() {
    let (app, data_dir) = device("style-forget-here", &[], None);
    let book = book();
    let gone = style("Old", "");
    in_step(&book, "s-gone", "rec-7", 2, &gone);
    let transport = Scripted::new(&[
        (200, &listing(&[record("rec-7", 2, &gone)], &[])),
        (200, tombstone()),
    ]);

    sync(&app, &transport, &book);

    let sent = transport.requests();
    assert_eq!(sent.len(), 2);
    assert_eq!(sent[1].method, Method::Delete);
    assert!(
        sent[1].url.ends_with("/writing-styles/rec-7?version=2"),
        "{}",
        sent[1].url
    );
    assert!(book.style("s-gone").is_none());
    assert!(
        app.app.syncable_writing_styles().is_empty(),
        "not brought straight back"
    );
    let _ = std::fs::remove_dir_all(data_dir);
}

/// A service that cannot be told keeps the record's claim: the style is not brought back here,
/// and the next pass tells the service again.
#[test]
fn a_style_forgotten_here_while_the_service_cannot_hear_is_never_brought_back() {
    let (app, data_dir) = device("style-forget-unheard", &[], None);
    let book = book();
    let gone = style("Old", "");
    in_step(&book, "s-gone", "rec-7", 2, &gone);
    let transport = Scripted::new(&[
        (200, &listing(&[record("rec-7", 2, &gone)], &[])),
        (503, "{}"),
    ]);

    sync(&app, &transport, &book);

    assert!(
        book.style("s-gone").is_some(),
        "kept, to tell the service next time"
    );
    assert!(app.app.syncable_writing_styles().is_empty());
    let _ = std::fs::remove_dir_all(data_dir);
}

/// Forgotten here after another device changed it: the edit made there outlives the forget made
/// here, and the style arrives again with that edit.
#[test]
fn a_style_forgotten_here_but_changed_elsewhere_comes_back() {
    let (app, data_dir) = device("style-forget-changed", &[], None);
    let book = book();
    in_step(&book, "s-gone", "rec-7", 2, &style("Old", ""));
    let changed = style("Old", "Edited on the phone.");
    let current = record("rec-7", 3, &changed);
    let refused = json!({ "defined": true, "code": "CONFLICT", "status": 409,
                          "data": { "current": current } })
    .to_string();
    let transport = Scripted::new(&[
        (200, &listing(std::slice::from_ref(&current), &[])),
        (409, &refused),
    ]);

    sync(&app, &transport, &book);

    let here = app.app.syncable_writing_styles();
    assert_eq!(here.len(), 1);
    assert_eq!(here[0].guide.notes, "Edited on the phone.");
    assert!(book.style("s-gone").is_none());
    assert_eq!(book.style(&here[0].id).unwrap().version, 3);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn a_style_forgotten_elsewhere_is_forgotten_here_with_every_account_s_choice_of_it() {
    let work = style("Work", "Short.");
    let (app, data_dir) = device("style-forget-elsewhere", &[("s1", &work)], None);
    app.set_account_writing_style("acct-1".to_owned(), Some("s1".to_owned()));
    assert_eq!(
        app.resolve_writing_style("acct-1".to_owned()).as_deref(),
        Some("s1")
    );
    let book = book();
    in_step(&book, "s1", "rec-1", 3, &work);
    let transport = Scripted::new(&[(200, &listing(&[], &[("rec-1", 4)]))]);

    sync(&app, &transport, &book);

    assert_eq!(transport.requests().len(), 1);
    assert!(app.app.syncable_writing_styles().is_empty());
    assert_eq!(app.resolve_writing_style("acct-1".to_owned()), None);
    assert!(book.style("s1").is_none());
    let _ = std::fs::remove_dir_all(data_dir);
}

/// Past the service's cap every further create is refused alike, so the pass stops asking.
#[test]
fn a_full_service_stops_the_uploads_for_the_pass() {
    let (a, b) = (style("A", ""), style("B", ""));
    let (app, data_dir) = device("style-cap", &[("s1", &a), ("s2", &b)], None);
    let too_many = r#"{"defined":true,"code":"BAD_REQUEST","status":400,
        "data":{"code":"too_many_styles"}}"#;
    let transport = Scripted::new(&[(200, &listing(&[], &[])), (400, too_many)]);
    let book = book();

    sync(&app, &transport, &book);

    assert_eq!(
        transport.requests().len(),
        2,
        "one create, and no second knock"
    );
    assert!(book.styles().is_empty());
    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn a_sign_in_without_the_style_scopes_skips_the_style_pass_silently() {
    let scopes = [
        "mailcal:entitlement:read",
        "mailcal:accounts:read",
        "mailcal:accounts:write",
    ];
    let work = style("Work", "");
    let (app, data_dir) = device(
        "style-no-scope",
        &[("s1", &work)],
        Some(grant(&scopes).as_str()),
    );
    let transport = Scripted::new(&[]);
    let book = book();

    assert!(sync(&app, &transport, &book).is_empty());
    assert!(transport.requests().is_empty());
    let _ = std::fs::remove_dir_all(data_dir);
}

/// Reading without writing: what arrives is taken, and nothing of this device's is sent.
#[test]
fn a_sign_in_that_may_only_read_styles_takes_but_never_sends() {
    let scopes = ["mailcal:accounts:read", "mailcal:writing-styles:read"];
    let work = style("Work", "");
    let (app, data_dir) = device(
        "style-read-only",
        &[("s1", &work)],
        Some(grant(&scopes).as_str()),
    );
    let arrival = style("Personal", "");
    let transport = Scripted::new(&[(200, &listing(&[record("rec-9", 1, &arrival)], &[]))]);
    let book = book();

    sync(&app, &transport, &book);

    assert_eq!(transport.requests().len(), 1, "no upload");
    assert_eq!(app.app.syncable_writing_styles().len(), 2);
    assert!(book.style("s1").is_none());
    let _ = std::fs::remove_dir_all(data_dir);
}

/// Keeping this device's version writes it over the service's **current** version, which the
/// pass that reported the conflict may no longer have been looking at.
#[test]
fn keeping_this_device_s_version_writes_it_over_the_service_s_current_one() {
    let mine = style("Work (laptop)", "Short.");
    let (app, data_dir) = device("style-keep-mine", &[("s1", &mine)], None);
    let book = book();
    in_step(&book, "s1", "rec-1", 3, &style("Work", "Short."));
    let stored = record("rec-1", 6, &mine).to_string();
    let transport = Scripted::new(&[
        (
            200,
            &listing(&[record("rec-1", 5, &style("Work (phone)", ""))], &[]),
        ),
        (200, &stored),
    ]);

    resolve(&app, &transport, &book, true).unwrap();

    let sent = transport.requests();
    assert_eq!(sent[1].method, Method::Put);
    let body = body_of(&sent[1]);
    assert_eq!(body["version"], 5);
    assert_eq!(body["style"]["name"], "Work (laptop)");
    let entry = book.style("s1").unwrap();
    assert_eq!(
        (entry.version, entry.fingerprint.clone()),
        (6, fingerprint(&mine))
    );
    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn keeping_the_other_devices_version_applies_it_here_and_sends_nothing() {
    let (app, data_dir) = device(
        "style-keep-theirs",
        &[("s1", &style("Work (laptop)", ""))],
        None,
    );
    let book = book();
    in_step(&book, "s1", "rec-1", 3, &style("Work", ""));
    let theirs = style("Work (phone)", "From the phone.");
    let transport = Scripted::new(&[(200, &listing(&[record("rec-1", 5, &theirs)], &[]))]);

    resolve(&app, &transport, &book, false).unwrap();

    assert_eq!(transport.requests().len(), 1);
    let here = app.app.syncable_writing_styles();
    assert_eq!(here[0].name, "Work (phone)");
    assert_eq!(passages_of(&data_dir, "s1")["languages"]["nl"][0], PASSAGE);
    assert_eq!(book.style("s1").unwrap().version, 5);
    let _ = std::fs::remove_dir_all(data_dir);
}
