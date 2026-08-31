//! Settings → Diagnostics over the rotating Linux log sink.

use std::{fs, path::PathBuf};

use adw::prelude::*;
use mailcal_bindings::LogLevel;

use super::{PageContext, dialog_box, group, page_box, pages::show_text_at_end};
use crate::{l10n, logger::diagnostic_log_path};

pub(super) fn diagnostics(ctx: &PageContext) -> gtk::Box {
    let content = page_box(l10n::settings_category_diagnostics());
    let log = group(
        l10n::diagnostics_log_heading(),
        l10n::diagnostics_log_description(),
    );
    let path = diagnostic_log_path();
    let (size, backups) = log_stats(&path);
    let status = adw::ActionRow::builder()
        .title(l10n::diagnostics_log_size_label())
        .subtitle(format!(
            "{} · {}: {backups}",
            gtk::glib::format_size(size),
            l10n::diagnostics_log_backups_label()
        ))
        .use_markup(false)
        .build();
    log.add(&status);
    log.add(
        &adw::ActionRow::builder()
            .title(l10n::diagnostics_log_cap_note())
            .use_markup(false)
            .build(),
    );
    let view = action(l10n::diagnostics_view_log());
    let button = gtk::Button::with_label(l10n::diagnostics_view_log());
    let parent = ctx.window.clone();
    let view_path = path.clone();
    button.connect_clicked(move |_| {
        let text = fs::read_to_string(&view_path)
            .unwrap_or_else(|_| l10n::diagnostics_log_empty().to_owned());
        show_text_at_end(&parent, l10n::diagnostics_log_heading(), &text);
    });
    view.add_suffix(&button);
    log.add(&view);
    let copy = action(l10n::diagnostics_copy_path());
    let button = gtk::Button::with_label(l10n::diagnostics_copy_path());
    let copy_path = path.clone();
    button.connect_clicked(move |_| {
        if let Some(display) = gtk::gdk::Display::default() {
            display
                .clipboard()
                .set_text(copy_path.to_string_lossy().as_ref());
        }
    });
    copy.add_suffix(&button);
    log.add(&copy);
    let export = action(l10n::diagnostics_export_log());
    let button = gtk::Button::with_label(l10n::diagnostics_export_log());
    let parent = ctx.window.clone();
    button.connect_clicked(move |_| confirm_export(&parent, path.clone()));
    export.add_suffix(&button);
    log.add(&export);
    content.append(&log);

    let detail = group(
        l10n::diagnostics_debug_heading(),
        l10n::diagnostics_debug_description(),
    );
    let debug = adw::SwitchRow::builder()
        .title(l10n::diagnostics_debug_heading())
        .subtitle(l10n::diagnostics_debug_description())
        .active(ctx.preferences.diagnostics_debug())
        .use_markup(false)
        .build();
    let app = ctx.app.clone();
    let preferences = ctx.preferences.clone();
    debug.connect_active_notify(move |row| {
        let enabled = row.is_active();
        preferences.set_diagnostics_debug(enabled);
        app.set_log_level(if enabled {
            LogLevel::Debug
        } else {
            LogLevel::Info
        });
    });
    detail.add(&debug);
    content.append(&detail);
    content
}

fn action(title: &str) -> adw::ActionRow {
    adw::ActionRow::builder()
        .use_markup(false)
        .title(title)
        .build()
}

fn log_stats(path: &PathBuf) -> (u64, u8) {
    let mut size = fs::metadata(path).map_or(0, |metadata| metadata.len());
    let mut backups = 0;
    for index in 1..=3 {
        let backup = PathBuf::from(format!("{}.{index}", path.to_string_lossy()));
        if let Ok(metadata) = fs::metadata(backup) {
            size = size.saturating_add(metadata.len());
            backups += 1;
        }
    }
    (size, backups)
}

fn confirm_export(parent: &gtk::Window, source: PathBuf) {
    let (dialog, _) =
        crate::ui::modal::new(parent, l10n::diagnostics_share_confirm_title(), 460, None);
    let content = dialog_box();
    let note = gtk::Label::new(Some(l10n::diagnostics_share_privacy_note()));
    note.set_wrap(true);
    note.set_xalign(0.0);
    content.append(&note);
    let actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    actions.set_halign(gtk::Align::End);
    let cancel = gtk::Button::with_label(l10n::action_cancel());
    let window = dialog.clone();
    cancel.connect_clicked(move |_| window.close());
    actions.append(&cancel);
    let export = gtk::Button::with_label(l10n::diagnostics_export_log());
    export.add_css_class("suggested-action");
    let chooser_parent = parent.clone();
    let window = dialog.clone();
    export.connect_clicked(move |_| {
        window.close();
        export_log(&chooser_parent, source.clone());
    });
    actions.append(&export);
    content.append(&actions);
    dialog.set_child(Some(&content));
    dialog.present();
}

fn export_log(parent: &gtk::Window, source: PathBuf) {
    #[cfg(debug_assertions)]
    if let Some(destination) = std::env::var_os("MAILCAL_DIAGNOSTICS_EXPORT_PATH") {
        let _ = copy_log(&source, &PathBuf::from(destination));
        return;
    }
    let dialog = gtk::FileDialog::builder()
        .initial_name("mailcal.log")
        .build();
    let parent = parent.clone();
    gtk::glib::MainContext::default().spawn_local(async move {
        if let Ok(file) = dialog.save_future(Some(&parent)).await
            && let Some(destination) = file.path()
        {
            let _ = copy_log(&source, &destination);
        }
    });
}

fn copy_log(source: &PathBuf, destination: &PathBuf) -> std::io::Result<u64> {
    fs::copy(source, destination)
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::{copy_log, log_stats};

    #[test]
    fn total_size_includes_the_three_rotated_backups() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock after epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("mailcal-log-stats-{nonce}.log"));
        fs::write(&path, b"12").expect("write live log");
        fs::write(format!("{}.1", path.to_string_lossy()), b"345").expect("write backup");

        assert_eq!(log_stats(&path), (5, 1));
    }

    #[test]
    fn export_copies_the_current_log_to_the_chosen_destination() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock after epoch")
            .as_nanos();
        let source = std::env::temp_dir().join(format!("mailcal-log-source-{nonce}.log"));
        let destination = std::env::temp_dir().join(format!("mailcal-log-export-{nonce}.log"));
        fs::write(&source, b"privacy-safe diagnostics").expect("write source log");

        copy_log(&source, &destination).expect("export log");

        assert_eq!(
            fs::read(destination).expect("read exported log"),
            b"privacy-safe diagnostics"
        );
    }
}
