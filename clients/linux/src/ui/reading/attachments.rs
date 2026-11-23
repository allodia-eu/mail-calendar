//! The reading pane's attachment rows: one per part, each naming the file its controls act on.

use adw::prelude::*;
use gtk::{accessible::Property as AccessibleProperty, glib};
use mailcal_bindings::AttachmentRow;

use super::{AppInput, ReadingSource};
use crate::l10n;

pub(super) fn attachment_row(
    attachment: &AttachmentRow,
    window: &gtk::Window,
    source: &ReadingSource,
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
    let reader = source.clone();
    open.connect_clicked(move |_| {
        input_sender.emit(AppInput::OpenAttachment {
            source: reader.clone(),
            id,
            file_name: file_name.clone(),
        });
    });
    row.add_suffix(&open);

    let save = attachment_button(l10n::action_save(), &attachment.file_name);
    let input_sender = sender.clone();
    let id = attachment.id;
    let file_name = attachment.file_name.clone();
    // Weak: the window holds this row.
    let parent = window.downgrade();
    let reader = source.clone();
    save.connect_clicked(move |_| {
        let Some(parent) = parent.upgrade() else {
            return;
        };
        let dialog = gtk::FileDialog::builder().initial_name(&file_name).build();
        let input_sender = input_sender.clone();
        let source = reader.clone();
        glib::MainContext::default().spawn_local(async move {
            if let Ok(file) = dialog.save_future(Some(&parent)).await
                && let Some(path) = file.path()
            {
                input_sender.emit(AppInput::SaveAttachment {
                    source,
                    id,
                    destination: path,
                });
            }
        });
    });
    row.add_suffix(&save);
    row
}

/// Save all, in the attachment group's header: every file into a folder the user picks. From the
/// folder being chosen until the files are written it holds, with a spinner beside its label
/// (`../../../docs/waiting.md`).
pub(super) fn save_all_button(
    window: &gtk::Window,
    source: &ReadingSource,
    sender: &relm4::Sender<AppInput>,
) -> gtk::Button {
    let button = gtk::Button::with_label(l10n::action_save_all());
    // Its child is swapped while it runs, so the name is set rather than read from a label.
    button.update_property(&[AccessibleProperty::Label(l10n::action_save_all())]);
    button.set_valign(gtk::Align::Center);
    let input_sender = sender.clone();
    // Weak: the window holds this button.
    let parent = window.downgrade();
    let reader = source.clone();
    button.connect_clicked(move |button| {
        let Some(parent) = parent.upgrade() else {
            return;
        };
        let dialog = gtk::FileDialog::new();
        let input_sender = input_sender.clone();
        let source = reader.clone();
        let button = button.downgrade();
        glib::MainContext::default().spawn_local(async move {
            let Ok(folder) = dialog.select_folder_future(Some(&parent)).await else {
                return;
            };
            let Some(directory) = folder.path() else {
                return;
            };
            let (done, finished) = futures::channel::oneshot::channel();
            if let Some(button) = button.upgrade() {
                show_running(&button, true);
            }
            input_sender.emit(AppInput::SaveAllAttachments {
                source,
                directory,
                done,
            });
            let _ = finished.await;
            // The pane may have moved to another message meanwhile and dropped this button.
            if let Some(button) = button.upgrade() {
                show_running(&button, false);
            }
        });
    });
    button
}

/// Draws Save all as running (held, a spinner before its label) or as it was.
fn show_running(button: &gtk::Button, running: bool) {
    let text = gtk::Label::new(Some(l10n::action_save_all()));
    if running {
        let spinner = adw::Spinner::new();
        spinner.set_valign(gtk::Align::Center);
        let content = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        content.append(&spinner);
        content.append(&text);
        button.set_child(Some(&content));
    } else {
        button.set_child(Some(&text));
    }
    button.set_sensitive(!running);
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
