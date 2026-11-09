//! The message list's stylesheet. Split from `mailbox` to keep both under the line limit.

use std::sync::Once;

/// Unread is carried by **weight**, and every mail row states its own; never leaving it to
/// inherit. A conversation's messages are rendered *inside* the row holding the conversation's
/// weight, so a reply already read would otherwise be drawn bold under an unread thread.
///
/// What the weight reaches is the subject and the sender; a message's preview and the trailing
/// date, count and badges stay regular, because bolding every line of an unread mailbox turns it
/// into one solid block that distinguishes nothing.
///
/// The pane beside the list, the contacts surface and the composer's recipient fields draw from the
/// same sheet; the badge pill and recipient pill; so that a shell-wide shape is defined once.
const MAIL_LIST_CSS: &str = "row.mailcal-unread { font-weight: bold; }
row.mailcal-read { font-weight: normal; }
row.mailcal-thread-message .subtitle { font-weight: normal; }
row .mailcal-meta { font-weight: normal; }
row .mailcal-badge {
  font-weight: normal;
  border-radius: 9999px;
  padding: 0 6px;
  background-color: alpha(currentColor, 0.12);
}
.mailcal-pill {
  border-radius: 9999px;
  padding: 2px 4px 2px 10px;
  background-color: alpha(currentColor, 0.12);
}
.mailcal-pill button {
  min-width: 20px;
  min-height: 20px;
  padding: 0;
}
";

/// Installs the message list's stylesheet and icons once per display.
pub(in crate::ui) fn install_styles() {
    static INSTALLED: Once = Once::new();
    if let Some(display) = gtk::gdk::Display::default() {
        INSTALLED.call_once(|| {
            let provider = gtk::CssProvider::new();
            provider.load_from_string(MAIL_LIST_CSS);
            gtk::style_context_add_provider_for_display(
                &display,
                &provider,
                gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
            );
            super::icons::install(&display);
        });
    }
}
