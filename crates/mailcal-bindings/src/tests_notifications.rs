//! Settings → Notifications across the FFI: the page and the intent that changes it.

use std::sync::mpsc;

use crate::{
    Intent, LogLevel, MailcalApp, PreferenceIntent, Surface,
    tests::{ChannelObserver, NullLogger},
};

#[test]
fn new_mail_notifications_are_the_cores() {
    let (tx, rx) = mpsc::channel();
    let app = MailcalApp::new_demo(
        Box::new(ChannelObserver { tx }),
        Box::new(NullLogger),
        LogLevel::Info,
        "Etc/UTC".to_owned(),
    );
    assert!(app.notifications_page().new_mail, "on until turned off");
    while rx.try_recv().is_ok() {}

    app.dispatch(Intent::Preferences {
        intent: PreferenceIntent::SetNewMailNotifications { on: false },
    });
    assert!(!app.notifications_page().new_mail);
    assert!(
        rx.try_iter()
            .any(|surface| matches!(surface, Surface::Settings))
    );
    assert!(app.collect_cached_new_mail().accounts.is_empty());
}
