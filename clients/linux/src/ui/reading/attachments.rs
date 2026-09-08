//! The reading pane's attachment rows: one per part, each naming the file its controls act on.

use adw::prelude::*;
use gtk::{accessible::Property as AccessibleProperty, glib};
use mailcal_bindings::AttachmentRow;

use super::AppInput;
use crate::l10n;

pub(super) fn attachment_row(
    attachment: &AttachmentRow,
    window: &adw::ApplicationWindow,
    sender: &relm4::Sender<AppInput>,
) -> adw::ActionRow {
    // Setters, not the property builder: `g_object_new` applies properties in its own order, so
    // `.use_markup(false)` written last still lands after the title: and a file called
    // `Q3 & Q4.pdf` has already been parsed as markup once by then (`../../../AGENTS.md` →
    // Client conventions). The name is the sender's text.
    let row = crate::ui::mailbox::plain_text_row();
    row.set_title(&attachment.file_name);
    row.set_subtitle(&format!(
        "{} · {}",
        attachment.media_type,
        glib::format_size(attachment.size)
    ));
    let open = attachment_button(l10n::action_open(), &attachment.file_name);
    let input_sender = sender.clone();
    let id = attachment.id;
    let file_name = attachment.file_name.clone();
    open.connect_clicked(move |_| {
        input_sender.emit(AppInput::OpenAttachment {
            id,
            file_name: file_name.clone(),
        });
    });
    row.add_suffix(&open);

    let save = attachment_button(l10n::action_save(), &attachment.file_name);
    let input_sender = sender.clone();
    let id = attachment.id;
    let file_name = attachment.file_name.clone();
    let parent = window.clone();
    save.connect_clicked(move |_| {
        let dialog = gtk::FileDialog::builder().initial_name(&file_name).build();
        let input_sender = input_sender.clone();
        let parent = parent.clone();
        glib::MainContext::default().spawn_local(async move {
            if let Ok(file) = dialog.save_future(Some(&parent)).await
                && let Some(path) = file.path()
            {
                input_sender.emit(AppInput::SaveAttachment {
                    id,
                    destination: path,
                });
            }
        });
    });
    row.add_suffix(&save);
    row
}

/// One attachment's Open or Save, named for the file it acts on.
///
/// A message can carry several, so the bare verb is the same word repeated down the list; and it
/// collides with every other Save on screen. The pattern is the recipient pill's.
pub(super) fn attachment_button(label: &str, file_name: &str) -> gtk::Button {
    let button = gtk::Button::with_label(label);
    // Description, not Label: a button built with a label carries a `labelled-by` relation to it,
    // which wins over the Label property; so the name stays the verb whatever we set, and the
    // file has to arrive as the supplementary half (`../../../docs/calendar.md` §4 notes the same
    // split). A message can carry several attachments, and "Save, button" three times over says
    // nothing about which file.
    button.update_property(&[AccessibleProperty::Description(file_name)]);
    button
}
