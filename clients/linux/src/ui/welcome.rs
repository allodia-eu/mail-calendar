//! First-run analytics consent, shown before account setup or notification prompts.

use adw::prelude::*;
use mailcal_bindings::MailcalApp;

use super::AppInput;
use crate::l10n;

pub(super) struct InitialFlow {
    pub(super) welcome: bool,
    pub(super) setup: bool,
    pub(super) setup_after_welcome: bool,
}

pub(super) const fn initial_flow(
    production: bool,
    requires_setup: bool,
    consent_asked: bool,
) -> InitialFlow {
    let welcome = production && !consent_asked;
    InitialFlow {
        welcome,
        setup: requires_setup && !welcome,
        setup_after_welcome: requires_setup && welcome,
    }
}

/// Lets the semantic runtime suite exercise first-run consent without opening a real keyring.
#[cfg(debug_assertions)]
pub(super) fn force_in_fixture() -> bool {
    std::env::var_os("MAILCAL_FORCE_ANALYTICS_WELCOME").is_some()
}

#[cfg(not(debug_assertions))]
pub(super) const fn force_in_fixture() -> bool {
    false
}

#[derive(Debug, Default)]
pub(super) struct WelcomeWindow {
    window: Option<gtk::Window>,
}

impl WelcomeWindow {
    pub(super) fn render(
        &mut self,
        pending: bool,
        parent: &adw::ApplicationWindow,
        app: Option<&MailcalApp>,
        sender: relm4::Sender<AppInput>,
    ) {
        if !pending {
            if let Some(window) = self.window.take() {
                // `close()` emits `close-request`, which this required first-run window rejects.
                // Destroying is the host-controlled completion path; the guard still blocks a
                // user trying to close the window before making a consent choice.
                window.destroy();
            }
            return;
        }
        if self.window.is_some() {
            return;
        }
        let (window, _) = crate::ui::modal::new(parent, l10n::welcome_title(), 540, Some(520));
        window.connect_close_request(|_| gtk::glib::Propagation::Stop);
        let content = gtk::Box::new(gtk::Orientation::Vertical, 16);
        content.set_margin_start(32);
        content.set_margin_end(32);
        content.set_margin_top(32);
        content.set_margin_bottom(32);
        let tagline = label(l10n::welcome_tagline());
        tagline.add_css_class("title-3");
        content.append(&tagline);
        content.append(&label(l10n::welcome_analytics_body()));
        let usage_switch = adw::SwitchRow::builder()
            .title(l10n::welcome_analytics_toggle())
            .active(false)
            .use_markup(false)
            .build();
        // A row is a `GtkListBoxRow`, and one appended straight to a box belongs to no list.
        // GTK's focus walk still reaches it, `gtk_list_box_row_grab_focus` then fails its own
        // precondition, and the row is skipped; so the consent choice is reachable by mouse
        // alone, and every launch leaves a critical in the diagnostic log.
        let usage_group = adw::PreferencesGroup::new();
        usage_group.add(&usage_switch);
        content.append(&usage_group);
        if let Some(app) = app {
            let preview = gtk::Button::with_label(l10n::welcome_analytics_preview());
            let parent = window.clone();
            let json = app.analytics_payload_preview();
            preview.connect_clicked(move |_| show_preview(&parent, &json));
            preview.set_halign(gtk::Align::Start);
            content.append(&preview);
        }
        let privacy = gtk::LinkButton::with_label(
            l10n::welcome_privacy_url(),
            l10n::welcome_privacy_policy(),
        );
        privacy.set_halign(gtk::Align::Start);
        content.append(&privacy);
        let start = gtk::Button::with_label(l10n::welcome_get_started());
        start.add_css_class("suggested-action");
        start.set_halign(gtk::Align::End);
        let input = sender;
        let choice = usage_switch.clone();
        start.connect_clicked(move |_| {
            input.emit(AppInput::AnalyticsDecided(choice.is_active()));
        });
        content.append(&start);
        window.set_child(Some(&content));
        window.present();
        self.window = Some(window);
    }

    #[cfg(test)]
    pub(super) fn current_window(&self) -> Option<gtk::Window> {
        self.window.clone()
    }
}

fn show_preview(parent: &gtk::Window, json: &str) {
    let (window, _) =
        crate::ui::modal::new(parent, l10n::welcome_analytics_preview(), 620, Some(460));
    let content = gtk::Box::new(gtk::Orientation::Vertical, 12);
    content.set_margin_start(18);
    content.set_margin_end(18);
    content.set_margin_top(18);
    content.set_margin_bottom(18);
    let view = gtk::TextView::new();
    view.set_editable(false);
    view.set_monospace(true);
    view.buffer().set_text(json);
    let scroll = gtk::ScrolledWindow::new();
    scroll.set_vexpand(true);
    scroll.set_child(Some(&view));
    content.append(&scroll);
    let close = gtk::Button::with_label(l10n::action_close());
    close.set_halign(gtk::Align::End);
    let dialog = window.clone();
    close.connect_clicked(move |_| dialog.close());
    content.append(&close);
    window.set_child(Some(&content));
    window.present();
}

fn label(text: &str) -> gtk::Label {
    let label = gtk::Label::new(Some(text));
    label.set_wrap(true);
    label.set_xalign(0.0);
    label
}

#[cfg(test)]
mod tests {
    use super::initial_flow;

    #[test]
    fn first_run_consent_precedes_required_account_setup() {
        let first_run = initial_flow(true, true, false);
        assert!(first_run.welcome);
        assert!(!first_run.setup);
        assert!(first_run.setup_after_welcome);

        let returning = initial_flow(true, true, true);
        assert!(!returning.welcome);
        assert!(returning.setup);
        assert!(!returning.setup_after_welcome);
    }
}
