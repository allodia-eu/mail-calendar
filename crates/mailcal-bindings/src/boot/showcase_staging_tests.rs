//! What `MAILCAL_OFFER_AI` puts in front of the UI suites: the writing assistant offered on the
//! in-memory boots, which offer it to nobody otherwise, so a store capture never shows it
//! (`docs/ai.md` rule 5); and nothing it would send leaves the device.

use std::{sync::Arc, time::Duration};

use mailcal_ai::HttpRequest;
use mailcal_app::ai::{AiCancel, AiHost as _};

use crate::{
    LogLevel, MailcalApp, ShowcaseLocale,
    ai_host::BindingsAiHost,
    credential_store::NoStoredCredentials,
    tests::{ChannelObserver, NullLogger},
};

fn in_memory_boots() -> [(&'static str, Arc<MailcalApp>); 3] {
    let boot = || {
        let (tx, _rx) = std::sync::mpsc::channel();
        (
            Box::new(ChannelObserver { tx }),
            Box::new(NullLogger),
            LogLevel::Info,
            "Europe/Amsterdam".to_owned(),
        )
    };
    let (observer, logger, level, zone) = boot();
    let showcase = MailcalApp::new_showcase(observer, logger, level, zone, ShowcaseLocale::En);
    let (observer, logger, level, zone) = boot();
    let first_run =
        MailcalApp::new_showcase_first_run(observer, logger, level, zone, ShowcaseLocale::En);
    let (observer, logger, level, zone) = boot();
    let demo = MailcalApp::new_demo(observer, logger, level, zone);
    [
        ("showcase", showcase),
        ("first run", first_run),
        ("demo", demo),
    ]
}

#[test]
fn an_in_memory_boot_offers_the_assistant_only_when_asked() {
    for (name, app) in in_memory_boots() {
        assert!(!app.ai_offer(), "{name}: offered unasked");
        assert!(app.ai_endpoint_form().is_none(), "{name}");
        assert!(app.writing_style_page().is_none(), "{name}");
        assert!(
            app.app.writing_styles().is_empty(),
            "{name}: nothing staged unasked"
        );

        super::offer_ai(&app);

        assert!(app.ai_offer(), "{name}: not offered when asked");
        assert!(app.ai_endpoint_form().is_some(), "{name}");
        assert!(app.writing_style_page().is_some(), "{name}");
    }
}

#[test]
fn the_offered_assistant_sends_nothing() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let host = BindingsAiHost::offline(
        runtime.handle().clone(),
        Arc::new(NoStoredCredentials),
        super::ai::transport,
    );
    let sent = host.transport(AiCancel::default()).post_json(HttpRequest {
        url: "https://models.example.eu/v1/chat/completions",
        bearer: None,
        body: "{}",
        timeout: Duration::from_secs(1),
    });
    assert!(
        sent.is_err(),
        "an offline host answers every request but the staged draft as no answer"
    );
}
