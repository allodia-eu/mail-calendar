//! Settings → Advanced across the FFI: the assistant access panel follows what the server is
//! doing and what the person chose, and the reset says it runs and what it came to.

use std::{
    sync::{Arc, mpsc},
    time::{Duration, Instant},
};

use crate::{
    ControlState, Intent, LogLevel, MailcalApp, McpHost, McpIntent, McpPanel, McpStatus,
    ResetOutcome, ShowcaseLocale, Surface,
    tests::{ChannelObserver, NullLogger},
};

fn showcase() -> (Arc<MailcalApp>, mpsc::Receiver<Surface>) {
    let (tx, rx) = mpsc::channel();
    let app = MailcalApp::new_showcase(
        Box::new(ChannelObserver { tx }),
        Box::new(NullLogger),
        LogLevel::Info,
        "Europe/Amsterdam".to_owned(),
        ShowcaseLocale::En,
    );
    (app, rx)
}

/// An endpoint of this test's own: a socket in a fresh directory, or a pipe named after it.
fn host(name: &str) -> McpHost {
    #[cfg(windows)]
    let endpoint = format!(r"\\.\pipe\mailcal-test-{name}-{}", std::process::id());
    #[cfg(not(windows))]
    let endpoint = crate::tests::temp_data_dir(name)
        .parent()
        .expect("the data directory's own")
        .join("mcp.sock")
        .to_string_lossy()
        .into_owned();
    McpHost {
        endpoint,
        relay_command: "relay".to_owned(),
        relay_args: vec!["--quiet".to_owned()],
    }
}

fn panel(app: &MailcalApp) -> McpPanel {
    app.advanced_page()
        .assistants
        .expect("a host with an endpoint has the panel")
}

/// Waits until the panel's status is no longer `Starting`, as a client waits for the signal.
fn settled(app: &MailcalApp) -> McpStatus {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let status = panel(app).status;
        if status != McpStatus::Starting || Instant::now() > deadline {
            return status;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn mcp(app: &MailcalApp, intent: McpIntent) {
    app.dispatch(Intent::Mcp { intent });
}

#[test]
fn there_is_no_panel_until_a_host_sets_an_endpoint() {
    let (app, _rx) = showcase();
    assert_eq!(app.advanced_page().assistants, None);

    let host = host("panel");
    app.set_mcp_host(Some(host.clone()));
    let panel = panel(&app);
    assert!(!panel.enabled);
    assert_eq!(panel.status, McpStatus::Off);
    assert!(panel.nothing_exposed, "nothing is exposed to begin with");
    assert!(!panel.accounts.is_empty());
    assert!(panel.accounts.iter().all(|account| !account.exposed));
    assert!(!panel.allow_direct_send);
    assert!(panel.require_known_recipient);
    assert!(!panel.known_recipient_editable);
    assert_eq!(
        panel.snippet,
        mailcal_mcp::configuration_snippet("relay", &["--quiet".to_owned()], &host.endpoint)
    );
}

#[test]
fn each_choice_is_taken_up_by_the_panel_before_dispatch_returns() {
    let (app, _rx) = showcase();
    app.set_mcp_host(Some(host("choices")));
    let account = panel(&app).accounts[0].account_id.clone();

    mcp(
        &app,
        McpIntent::SetAccountExposed {
            account: account.clone(),
            exposed: true,
        },
    );
    let after = panel(&app);
    assert!(!after.nothing_exposed);
    assert!(
        after
            .accounts
            .iter()
            .any(|row| row.account_id == account && row.exposed)
    );

    mcp(&app, McpIntent::SetDirectSend { allow: true });
    assert!(panel(&app).known_recipient_editable);
    mcp(&app, McpIntent::SetKnownRecipient { require: false });
    assert!(!panel(&app).require_known_recipient);
    mcp(&app, McpIntent::SetDirectSend { allow: false });
    let after = panel(&app);
    assert!(!after.allow_direct_send);
    assert!(!after.known_recipient_editable);
    assert_eq!(after.status, McpStatus::Off, "none of this turns it on");
}

#[test]
fn turning_it_on_says_listening_once_bound_and_a_second_copy_says_another_has_it() {
    let (first, rx) = showcase();
    let host = host("listening");
    first.set_mcp_host(Some(host.clone()));
    while rx.try_recv().is_ok() {}

    mcp(&first, McpIntent::SetEnabled { enabled: true });
    assert_eq!(settled(&first), McpStatus::Listening);
    assert!(
        rx.try_iter()
            .any(|surface| matches!(surface, Surface::Settings)),
        "Settings hears it"
    );

    let (second, _rx) = showcase();
    second.set_mcp_host(Some(host));
    mcp(&second, McpIntent::SetEnabled { enabled: true });
    assert_eq!(settled(&second), McpStatus::AnotherInstance);
    assert_eq!(panel(&first).status, McpStatus::Listening);

    mcp(&second, McpIntent::SetEnabled { enabled: false });
    mcp(&first, McpIntent::SetEnabled { enabled: false });
    assert_eq!(panel(&first).status, McpStatus::Off);
}

#[test]
fn the_reset_runs_from_the_press_until_mail_is_back_then_says_it_is_done() {
    let (app, rx) = showcase();
    let idle = app.advanced_page();
    assert_eq!(
        idle.reset,
        ControlState {
            enabled: true,
            running: false
        }
    );
    assert_eq!(idle.reset_outcome, None);
    while rx.try_recv().is_ok() {}

    app.dispatch(Intent::ResetDatabase);
    let deadline = Instant::now() + Duration::from_secs(10);
    while app.advanced_page().reset_outcome.is_none() {
        assert!(Instant::now() < deadline, "the reset finishes");
        std::thread::sleep(Duration::from_millis(10));
    }
    let done = app.advanced_page();
    assert_eq!(
        done.reset,
        ControlState {
            enabled: true,
            running: false
        }
    );
    assert_eq!(done.reset_outcome, Some(ResetOutcome::Done));
    assert!(
        rx.try_iter()
            .any(|surface| matches!(surface, Surface::Settings)),
        "Settings hears it"
    );
}
