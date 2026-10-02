//! GTK4/libadwaita host for Allodia Mail & Calendar.

// `MailcalError` carries the certificate a server presented that did not verify
// (`docs/certificate-exceptions.md`), and `ConnectFailure` carries it onward, so every `Result`
// either appears in is past the lint's 128-byte threshold. The usual remedy is a `Box`, and the
// error's own crate cannot take one: a UniFFI record's fields are the wire format, so the
// indirection cannot be expressed (`crates/mailcal-bindings/Cargo.toml` carries the same allow
// for the same reason). Boxing it here instead would unwrap at every call across the boundary.
#![allow(clippy::result_large_err)]

use adw::prelude::*;

mod allodia_sync_store;
mod appearance;
mod boot;
mod crash;
mod dev_account;
mod host_runtime;
// Every client receives the full shared catalog and uses only its implemented surfaces.
// `match_same_arms` fires on the locale tables, where coinciding arms are a fact about translations
//: "Vista" is Spanish, Italian and Portuguese for "View": and the only repair would be in a file
// nobody edits. Hand-written modules keep the lint.
#[allow(dead_code, clippy::match_same_arms)]
mod l10n {
    include!(concat!(env!("OUT_DIR"), "/l10n.rs"));
}
mod logger;
mod mail_link;
mod observer;
mod preferences;
mod secrets;
mod share;
mod showcase;
mod ui;

static APP_BROKER: relm4::MessageBroker<ui::AppInput> = relm4::MessageBroker::new();

/// The flags the one application is built with.
///
/// `NON_UNIQUE` is deliberately absent: a second launch is handed to the process already running
/// (`connect_command_line` below, which then activates it), and a second process would be a second
/// core over the same SQLite store (`docs/reading-window.md`). The mailbox, the reading windows and
/// the composer windows are views of one app; nothing here may make a second one.
const APPLICATION_FLAGS: gtk::gio::ApplicationFlags =
    gtk::gio::ApplicationFlags::HANDLES_COMMAND_LINE;

/// Claim the application id as the program name, so every window this process opens reaches the
/// desktop as this app's.
///
/// GTK takes a window's Wayland `app_id` from the `GtkApplication` the window belongs to, and from
/// the program name for every window that belongs to none. The reading, composer and settings
/// windows are deliberately not application windows (`docs/reading-window.md`), so without this
/// they arrive as `mailcal-linux`, which matches no desktop entry: the shell files them under a
/// second application, with neither the app's name nor its icon, and its window switcher will not
/// move between them and the mailbox.
fn claim_desktop_identity() {
    gtk::glib::set_program_name(Some(l10n::APP_ID));
}

fn main() {
    claim_desktop_identity();

    // A showcase run pins the language for the session only, above the stored choice and without
    // touching it; a capture must never rewrite the developer's own preference. Compiled out of a
    // release build with the rest of `showcase`.
    #[cfg(any(debug_assertions, feature = "dev-harness"))]
    let language = showcase::language_override().or_else(|| preferences::global().language());
    #[cfg(not(any(debug_assertions, feature = "dev-harness")))]
    let language = preferences::global().language();
    l10n::set_locale_override(language.as_deref());

    // Refuse a screen this client cannot reach, before the window exists. The capture script proves
    // a run reached the fictional dataset by finding the core's showcase marker in the log; exiting
    // here writes no marker, so the run fails at the launch instead of filing a photograph of the
    // wrong screen under the requested name.
    #[cfg(any(debug_assertions, feature = "dev-harness"))]
    if showcase::is_on()
        && let Err(name) = showcase::screen()
    {
        eprintln!(
            "MAILCAL_SHOWCASE_SCREEN={name} is not a screen this client can reach \
             (list, reply, settings, add-account, calendar, invitation, signatures)"
        );
        std::process::exit(2);
    }

    // Same reason, one switch over: a fixture this client cannot boot must stop the launch rather
    // than fall through to the developer's stored accounts, which is what an operator who asked
    // for the harness would read as a harness holding no mail.
    #[cfg(any(debug_assertions, feature = "dev-harness"))]
    if let Err(problem) = boot::check_dev_account() {
        eprintln!("{problem}");
        std::process::exit(2);
    }

    let application = adw::Application::builder()
        .application_id(l10n::APP_ID)
        .flags(APPLICATION_FLAGS)
        .build();
    application.connect_command_line(|application, command_line| {
        let arguments = command_line.arguments();
        // A mail link first: it is the narrower question, and a `mailto:` argument is not a file,
        // so asking the other way round would let a link fall through to the share parser.
        if let Some(prefill) = mail_link::prefill_arguments(&arguments) {
            log::info!("mail link received");
            APP_BROKER.send(ui::AppInput::OpenMailto(Box::new(prefill)));
        } else if let Some(prefill) = share::prefill_arguments(&arguments) {
            log::info!(
                "share received: {} file(s), {} refused",
                prefill.attachments.len(),
                prefill.rejected.len()
            );
            APP_BROKER.send(ui::AppInput::OpenShare(Box::new(prefill)));
        }
        application.activate();
        gtk::glib::ExitCode::SUCCESS
    });
    let app = relm4::RelmApp::<ui::AppInput>::from_app(application).with_broker(&APP_BROKER);
    app.run::<ui::AppModel>(());
}

#[cfg(test)]
mod tests {
    use super::{APPLICATION_FLAGS, claim_desktop_identity};

    /// Every window, including the ones that belong to no `GtkApplication`, reaches the desktop as
    /// this app rather than as the binary that happens to draw it.
    #[test]
    fn every_window_this_process_opens_is_filed_under_the_app() {
        claim_desktop_identity();
        assert_eq!(
            gtk::glib::program_name().as_deref(),
            Some(crate::l10n::APP_ID)
        );
    }

    /// A second launch reaches the running app rather than starting a second core.
    ///
    /// The flags rather than a built `GApplication`: registering one would claim this
    /// application's bus name, and the assertion is about what we ask for.
    #[test]
    fn a_second_launch_is_not_a_second_app() {
        assert!(
            !APPLICATION_FLAGS.contains(gtk::gio::ApplicationFlags::NON_UNIQUE),
            "a second process is a second connection to the same store"
        );
        assert!(
            APPLICATION_FLAGS.contains(gtk::gio::ApplicationFlags::HANDLES_COMMAND_LINE),
            "a mail link handed to the running app arrives on its command line"
        );
    }
}
