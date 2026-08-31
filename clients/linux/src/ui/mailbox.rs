//! Mailbox-list projection for the three-pane shell. The pane beside it is
//! [`super::folder_pane`], which shares this module's row skeleton and styles.

use std::{collections::HashSet, sync::Once};

use adw::prelude::*;
use gtk::accessible::Property as AccessibleProperty;
use mailcal_bindings::{FlatRow, MailboxListSnapshot, SnapshotRow, ThreadMessage, ThreadRow};

pub(super) use super::mailbox_display::MailboxRendering;
#[cfg(test)]
pub(super) use super::mailbox_display::display_row;
use super::{
    AppInput, avatar, mail_actions,
    mailbox_display::{flat_display, message_display, thread_display},
    model::OpenedMessage,
    row_action, timestamps,
};
#[cfg(test)]
use super::{mailbox_display::DisplayRow, mailbox_reconcile::reconcile};
use crate::l10n;

/// A conversation's identity in the list. Account-scoped, because a provider's thread id is
/// unique only *within* its account; two accounts in the unified view can carry the same one.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct ThreadKey(String);

impl ThreadKey {
    pub(super) fn new(account: &str, thread_id: &str) -> Self {
        Self(format!("{account}/{thread_id}"))
    }

    pub(super) fn of(row: &ThreadRow) -> Self {
        Self::new(&row.account, &row.thread_id)
    }
}

/// Renders the message list. A conversation is drawn as an expander whose sub-rows are the whole
/// thread; `expanded` is the host's record of which ones are open, so a refresh that rebuilds the
/// list leaves an open conversation open.
/// Brings the message list to `snapshot`, given the rows `previous` left on screen.
///
/// A row whose rendering is unchanged keeps the widget it already has, so a sync that commits
/// mail in chunks no longer tears down and rebuilds every row several times a second. `previous`
/// is empty on the first render, which builds every row.
#[cfg(test)]
pub(crate) fn render_messages(
    list: &gtk::ListBox,
    previous: &[DisplayRow],
    snapshot: &MailboxListSnapshot,
    expanded: &HashSet<ThreadKey>,
    in_junk_folder: bool,
    zone: &str,
    sender: &relm4::Sender<AppInput>,
) {
    install_styles();
    let next: Vec<DisplayRow> = snapshot
        .rows
        .iter()
        .map(|row| display_row(row, zone))
        .collect();
    reconcile(list, previous, &next, |index| {
        build_row(
            &snapshot.rows[index],
            expanded,
            in_junk_folder,
            zone,
            sender,
        )
    });
}

pub(super) fn build_row(
    row: &SnapshotRow,
    expanded: &HashSet<ThreadKey>,
    in_junk_folder: bool,
    zone: &str,
    sender: &relm4::Sender<AppInput>,
) -> gtk::Widget {
    match row {
        SnapshotRow::Flat { row } => flat_row(row, in_junk_folder, zone, sender).upcast(),
        SnapshotRow::Thread { row } => {
            let key = ThreadKey::of(row);
            thread_row(row, expanded.contains(&key), zone, sender).upcast()
        }
    }
}

/// The message a conversation row opens: its latest in-scope message, which is what the row
/// summarises. `None` once the conversation has left the list the key was recorded against.
pub(crate) fn thread_representative(
    snapshot: &MailboxListSnapshot,
    thread: &ThreadKey,
) -> Option<OpenedMessage> {
    snapshot.rows.iter().find_map(|row| match row {
        SnapshotRow::Thread { row } if &ThreadKey::of(row) == thread => {
            Some(OpenedMessage::from_thread(row))
        }
        _ => None,
    })
}

pub(super) fn clear(list: &gtk::ListBox) {
    while let Some(child) = list.first_child() {
        list.remove(&child);
    }
}

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

/// The bundled glyphs, in the resource layout `IconTheme::add_resource_path` expects.
/// Must equal the `prefix` in icons/mailcal.gresource.xml. It is a namespace inside the binary,
/// not an identity, so it does not follow the application id.
const ICON_RESOURCE_PATH: &str = "/mailcal/icons";

/// Installs the message list's stylesheet and icons once per display.
pub(super) fn install_styles() {
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
            gtk::gio::resources_register_include!("mailcal.gresource")
                .expect("the bundled icons are compiled into the binary");
            gtk::IconTheme::for_display(&display).add_resource_path(ICON_RESOURCE_PATH);
        });
    }
}

/// Builds a row whose text is plain, never Pango markup.
///
/// The flag has to be set **before** the text, and the property builder cannot promise that:
/// `g_object_new` applies properties in its own order, so `.subtitle(…).use_markup(false)` sets
/// the subtitle while markup is still on. libadwaita re-applies the labels when the flag flips,
/// so the row still *reads* correctly; but the first, markup-parsed attempt has already logged a
/// `Failed to set text … from markup` warning for every sender or subject containing an
/// ampersand. That noise lands in the diagnostic log a user attaches to a support request, where
/// it is indistinguishable from a real failure. Setters run in the order written.
pub(super) fn plain_text_row() -> adw::ActionRow {
    let row = adw::ActionRow::new();
    row.set_use_markup(false);
    row
}

/// A single message: subject over sender, with the date trailing.
fn flat_row(
    row: &FlatRow,
    in_junk_folder: bool,
    zone: &str,
    sender: &relm4::Sender<AppInput>,
) -> adw::ActionRow {
    let display = flat_display(row, zone);
    // Subjects and senders are the server's text: markup-shaped input must render as itself, and
    // a bare ampersand must not be read as an entity.
    let widget = plain_text_row();
    widget.set_title(&display.title);
    widget.set_subtitle(&display.subtitle);
    widget.set_title_lines(1);
    widget.set_subtitle_lines(1);
    widget.set_activatable(true);
    widget.add_css_class(weight_class(display.unread));
    widget.add_prefix(&avatar::unread_dot(display.unread));
    widget.add_prefix(&avatar::view(&display.avatar, 36));
    if display.flagged {
        widget.add_suffix(&flag_icon());
    }
    if display.has_attachment {
        widget.add_suffix(&attachment_icon());
    }
    widget.add_suffix(&meta_label(&timestamps::relative_date(&display.date, zone)));
    widget.add_suffix(&mail_actions::message_menu_button(
        row,
        in_junk_folder,
        sender,
    ));
    let opened = OpenedMessage {
        account: row.account.clone(),
        key: row.key.clone(),
        subject: row.subject.clone(),
        from: row.from.clone(),
        date: row.date.clone(),
        avatar: display.avatar,
    };
    let input = sender.clone();
    row_action::action_row(&widget, move || {
        input.emit(AppInput::OpenThreadMessage(Box::new(opened.clone())));
    });
    widget
}

/// A conversation: the summary the folder lists it by, expanding into every message on the thread
/// ; the account owner's own Sent replies included, which the core gathers across folders.
///
/// The disclosure arrow is libadwaita's, and an icon theme may replace it: libadwaita draws
/// `adw-expander-arrow-symbolic`: art that points **up**: and rotates it half a turn while
/// collapsed. **Yaru** ships that name with its own down-pointing chevron, so on an Ubuntu
/// desktop the rotation inverts it: up while collapsed, down while open, in every expander row
/// on the system. Not this row's doing, and not ours to correct by overriding the user's icons.
fn thread_row(
    row: &ThreadRow,
    expanded: bool,
    zone: &str,
    sender: &relm4::Sender<AppInput>,
) -> adw::ExpanderRow {
    let display = thread_display(row, zone);
    let widget = adw::ExpanderRow::new();
    // Before the text, for the reason `plain_text_row` gives.
    widget.set_use_markup(false);
    widget.set_title(&display.title);
    widget.set_subtitle(&display.subtitle);
    widget.set_title_lines(1);
    widget.set_subtitle_lines(1);
    widget.add_css_class(weight_class(display.unread));
    widget.add_prefix(&avatar::unread_dot(display.unread));
    widget.add_prefix(&avatar::view(&display.avatar, 36));
    if display.count > 1 {
        widget.add_suffix(&count_badge(display.count));
    }
    if display.has_attachment {
        widget.add_suffix(&attachment_icon());
    }
    widget.add_suffix(&meta_label(&timestamps::relative_date(&display.date, zone)));
    widget.add_suffix(&mail_actions::thread_menu_button(
        &row.account,
        &row.thread_id,
        sender,
    ));
    let opened = OpenedMessage::from_thread(row);
    let input = sender.clone();
    row_action::expander_row(&widget, move || {
        input.emit(AppInput::OpenThreadMessage(Box::new(opened.clone())));
    });
    for message in &row.messages {
        widget.add_row(&thread_message_row(row, message, zone, sender));
    }
    // Restore the disclosure the host recorded **before** listening for it, so rebuilding an open
    // conversation isn't reported back as a fresh toggle; which would re-open its message and
    // steal the reading pane from whichever sub-row the user is on.
    widget.set_expanded(expanded);
    let input = sender.clone();
    let thread = ThreadKey::of(row);
    widget.connect_expanded_notify(move |widget| {
        input.emit(AppInput::SetThreadExpanded {
            thread: thread.clone(),
            expanded: widget.is_expanded(),
        });
    });
    widget
}

/// One message of an expanded conversation: sender over preview, badged when the account owner
/// sent it. Activating it opens that message rather than the conversation's representative.
fn thread_message_row(
    thread: &ThreadRow,
    message: &ThreadMessage,
    zone: &str,
    sender: &relm4::Sender<AppInput>,
) -> adw::ActionRow {
    let display = message_display(message);
    let widget = plain_text_row();
    widget.set_title(&display.from);
    widget.set_subtitle(&display.preview);
    widget.set_title_lines(1);
    widget.set_subtitle_lines(1);
    widget.set_activatable(true);
    widget.add_css_class(weight_class(display.unread));
    widget.add_css_class("mailcal-thread-message");
    widget.add_prefix(&avatar::unread_dot(display.unread));
    widget.add_prefix(&avatar::view(&display.avatar, 32));
    if display.outgoing {
        widget.add_suffix(&badge(l10n::thread_sent()));
    }
    if display.has_attachment {
        widget.add_suffix(&attachment_icon());
    }
    widget.add_suffix(&meta_label(&timestamps::relative_date(&display.date, zone)));
    // The conversation's subject heads the reading view, and seeds a reply's; a message on a
    // thread carries no subject of its own here.
    let opened = OpenedMessage {
        account: message.account.clone(),
        key: message.key.clone(),
        subject: thread.subject.clone(),
        from: message.from.clone(),
        date: message.date.clone(),
        avatar: display.avatar,
    };
    let input = sender.clone();
    row_action::action_row(&widget, move || {
        input.emit(AppInput::OpenThreadMessage(Box::new(opened.clone())));
    });
    widget
}

fn attachment_icon() -> gtk::Image {
    let icon = gtk::Image::from_icon_name("mail-attachment-symbolic");
    icon.set_tooltip_text(Some(l10n::a11y_has_attachment()));
    icon
}

fn flag_icon() -> gtk::Image {
    let icon = gtk::Image::from_icon_name("starred-symbolic");
    icon.set_tooltip_text(Some(l10n::a11y_flagged()));
    icon.update_property(&[AccessibleProperty::Label(l10n::a11y_flagged())]);
    icon
}

/// A date, count or badge sits inside the row and would otherwise inherit an unread row's weight.
/// It is centred rather than filled, so a pill is the height of its text and not of the row.
fn meta_label(text: &str) -> gtk::Label {
    let label = trailing_label(text);
    label.add_css_class("dim-label");
    label.add_css_class("mailcal-meta");
    label
}

pub(super) fn badge(text: &str) -> gtk::Label {
    let label = trailing_label(text);
    label.add_css_class("mailcal-badge");
    label
}

fn trailing_label(text: &str) -> gtk::Label {
    let label = gtk::Label::new(Some(text));
    label.add_css_class("caption");
    label.set_valign(gtk::Align::Center);
    label
}

/// The conversation's length, as a pill. The digit alone is what the eye needs; the spoken and
/// hovered labels say what it counts.
fn count_badge(count: u32) -> gtk::Label {
    let label = badge(&count.to_string());
    let spoken = l10n::mailbox_count_messages(i64::from(count));
    label.set_tooltip_text(Some(&spoken));
    label.update_property(&[AccessibleProperty::Label(&spoken)]);
    label
}

fn weight_class(unread: bool) -> &'static str {
    if unread {
        "mailcal-unread"
    } else {
        "mailcal-read"
    }
}

#[cfg(test)]
#[path = "mailbox_tests.rs"]
pub(super) mod tests;

#[cfg(test)]
#[path = "mailbox_thread_tests.rs"]
mod thread_tests;

/// The tree-walking helper the widget tests read a rendered page with, shared with the
/// Settings tests: what a label *shows* is the only honest assertion, wherever the widget came
/// from.
#[cfg(test)]
pub(crate) use tests::rendered_labels;
