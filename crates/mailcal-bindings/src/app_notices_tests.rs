//! The notices as a host reaches them: the pull, the signal and the two intents.

use std::{
    sync::{Arc, mpsc},
    time::{Duration, Instant},
};

use crate::{
    Intent, LogLevel, MailcalApp, NoticeIntent, NoticeKind, Surface,
    tests::{ChannelObserver, NullLogger},
};

fn app() -> (Arc<MailcalApp>, mpsc::Receiver<Surface>) {
    let (tx, rx) = mpsc::channel();
    let app = MailcalApp::new_demo(
        Box::new(ChannelObserver { tx }),
        Box::new(NullLogger),
        LogLevel::Info,
        "Etc/UTC".to_owned(),
    );
    (app, rx)
}

/// Whether `Surface::Notices` reaches the host within a moment, draining what arrives before it.
fn notices_signalled(rx: &mpsc::Receiver<Surface>) -> bool {
    let deadline = Instant::now() + Duration::from_secs(2);
    while let Some(left) = deadline.checked_duration_since(Instant::now()) {
        match rx.recv_timeout(left) {
            Ok(Surface::Notices) => return true,
            Ok(_) => {}
            Err(_) => return false,
        }
    }
    false
}

fn go_offline(app: &MailcalApp) {
    app.dispatch(Intent::ReportNetworkReachable { reachable: false });
}

#[test]
fn going_offline_raises_a_notice_and_signals_it() {
    let (app, rx) = app();
    while rx.try_recv().is_ok() {}
    go_offline(&app);
    assert!(
        notices_signalled(&rx),
        "a connectivity change is a notices change"
    );
    let kinds: Vec<_> = app.notices().notices.into_iter().map(|n| n.kind).collect();
    assert_eq!(kinds, [NoticeKind::Offline]);
}

#[test]
fn a_notice_about_a_state_that_holds_is_not_closed_by_a_dismiss() {
    let (app, _rx) = app();
    go_offline(&app);
    let id = app.notices().notices[0].id.clone();
    app.dispatch(Intent::Notices {
        intent: NoticeIntent::Dismiss { id },
    });
    assert_eq!(app.notices().notices.len(), 1);
}

#[test]
fn a_failed_remedy_is_ignored_for_a_notice_with_none() {
    let (app, rx) = app();
    go_offline(&app);
    let id = app.notices().notices[0].id.clone();
    while rx.try_recv().is_ok() {}
    app.dispatch(Intent::Notices {
        intent: NoticeIntent::RemedyFailed { id },
    });
    assert!(!app.notices().notices[0].remedy_failed);
    assert!(
        !notices_signalled(&rx),
        "nothing changed, so nothing is signalled"
    );
}
