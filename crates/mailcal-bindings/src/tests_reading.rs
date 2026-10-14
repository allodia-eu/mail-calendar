//! The reading view's rendering across the FFI: the scheme a message is drawn in, and the
//! preference that chooses it (`docs/reading-dark-mode.md`).

use std::sync::mpsc;

use crate::{
    LogLevel, MailcalApp, MessageScheme, Surface, message_canvas, render_message_html,
    tests::{ChannelObserver, NullLogger},
};

#[test]
fn dark_messages_cross_the_ffi() {
    // The preference behind the reading pane's sun/moon toggle and Settings → Reading: on by
    // default, persisted by the core, and announced as a Settings change so every client
    // re-renders the open message (docs/reading-dark-mode.md).
    let (tx, rx) = mpsc::channel();
    let app = MailcalApp::new_demo(
        Box::new(ChannelObserver { tx }),
        Box::new(NullLogger),
        LogLevel::Info,
        "Etc/UTC".to_owned(),
    );
    assert!(app.display_settings().dark_message_background);
    while rx.try_recv().is_ok() {}
    app.set_dark_message_background(false);
    assert!(!app.display_settings().dark_message_background);
    assert!(
        rx.try_iter()
            .any(|surface| matches!(surface, Surface::Settings))
    );

    // The document and the canvas a client paints around it agree on the page in each scheme.
    let fragment = r#"<p style="color:#000000">x</p>"#.to_owned();
    for scheme in [MessageScheme::Light, MessageScheme::Dark] {
        let canvas = message_canvas(scheme);
        let doc = render_message_html(fragment.clone(), false, scheme);
        assert!(
            doc.contains(&format!("background:{}", canvas.background)),
            "{doc}"
        );
    }
    assert_ne!(
        message_canvas(MessageScheme::Light).background,
        message_canvas(MessageScheme::Dark).background
    );
    let dark = render_message_html(fragment, false, MessageScheme::Dark);
    assert!(!dark.contains("color:#000000"), "{dark}");
}
