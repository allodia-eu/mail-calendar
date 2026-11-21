//! Settings holds the writing assistant exactly while the core offers it (`docs/ai.md` rule 4).
//! The window is drawn from a core that offers it to nobody, an in-memory showcase: no Writing
//! style in the sidebar or among the pages, and Advanced without the own endpoint's form. A window
//! drawn again in place while it showed Writing style, which is what the offer going under it
//! does, opens on its first category.

use std::sync::Arc;

use adw::prelude::*;
use mailcal_bindings::{LogLevel, Logger, MailcalApp, Observer, ShowcaseLocale, Surface};

use super::{Category, Redraw, SettingsState, SettingsWindow, visible_categories};
use crate::{l10n, preferences::HostPreferences, ui::AppInput};

/// The offer moving either way draws the window again in place, on the page it showed, and a
/// read that finds it where it was draws nothing.
#[test]
fn the_offer_moving_redraws_settings_in_place() {
    let mut state = SettingsState::default();
    state.open(Some(Category::WritingStyle));
    let opened = state.render_state().generation;

    state.follow_ai_offer(true);
    let offered = state.render_state();
    assert_eq!(offered.generation, opened + 1);
    assert_eq!(offered.redraw, Redraw::InPlace, "a redraw, never an open");
    assert_eq!(offered.category, Category::WritingStyle);

    state.follow_ai_offer(true);
    assert_eq!(
        state.render_state().generation,
        opened + 1,
        "unchanged, nothing is drawn"
    );

    state.follow_ai_offer(false);
    let withdrawn = state.render_state();
    assert_eq!(withdrawn.generation, opened + 2);
    assert_eq!(withdrawn.redraw, Redraw::InPlace);
}

struct Quiet;

impl Observer for Quiet {
    fn surface_changed(&self, _: Surface) {}
}

impl Logger for Quiet {
    fn log(&self, _: LogLevel, _: String, _: String) {}
}

/// Every label under `root` a person could see: none inside a hidden widget.
fn shown_labels(root: &gtk::Widget) -> Vec<String> {
    let mut found = Vec::new();
    if !root.is_visible() {
        return found;
    }
    if let Some(label) = root.downcast_ref::<gtk::Label>() {
        found.push(label.text().to_string());
    }
    let mut child = root.first_child();
    while let Some(widget) = child {
        found.extend(shown_labels(&widget));
        child = widget.next_sibling();
    }
    found
}

/// Called from the crate's single `gtk::init` test.
pub(crate) fn not_offered_settings_holds_none_of_it(parent: &adw::ApplicationWindow) {
    let app = MailcalApp::new_showcase(
        Box::new(Quiet),
        Box::new(Quiet),
        LogLevel::Info,
        "Europe/Amsterdam".to_owned(),
        ShowcaseLocale::En,
    );
    assert!(
        !app.ai_offer(),
        "an in-memory showcase offers the assistant to nobody unless MAILCAL_OFFER_AI asks"
    );
    let scratch = tempfile::tempdir().expect("a scratch directory");
    let preferences = Arc::new(HostPreferences::open(scratch.path().join("host.json")));
    let (sender, _receiver) = relm4::channel::<AppInput>();
    let mut settings = SettingsWindow::default();
    let mut state = SettingsState::default();
    let first = visible_categories(false)[0];
    let render = |settings: &mut SettingsWindow, state: &SettingsState| {
        settings.render(
            state.render_state(),
            parent,
            Some(&app),
            Arc::clone(&preferences),
            sender.clone(),
        );
        while gtk::glib::MainContext::default().iteration(false) {}
    };

    // Asked for by name, as a remedy does, Writing style is not there to open.
    state.open(Some(Category::WritingStyle));
    render(&mut settings, &state);
    let pages = settings.pages.clone().expect("the window's pages");
    assert!(
        pages.child_by_name(Category::WritingStyle.name()).is_none(),
        "no Writing style page"
    );
    assert_eq!(
        pages.visible_child_name().as_deref(),
        Some(first.name()),
        "opened on the first category"
    );
    let window = settings.window.clone().expect("the window");
    let sidebar = shown_labels(window.upcast_ref());
    assert!(
        !sidebar
            .iter()
            .any(|text| text == l10n::settings_category_writing_style()),
        "no Writing style in the sidebar: {sidebar:?}"
    );

    let advanced = pages
        .child_by_name(Category::Advanced.name())
        .expect("Advanced");
    let shown = shown_labels(&advanced);
    assert!(
        shown
            .iter()
            .any(|text| text == l10n::action_reset_database()),
        "Advanced is drawn: {shown:?}"
    );
    assert!(
        !shown.iter().any(|text| text == l10n::ai_endpoint_title()),
        "Advanced has no own endpoint form: {shown:?}"
    );

    // The offer going while Writing style is open draws the window again in place on it; with
    // the page gone, the window is on its first category.
    state.record_category(Category::WritingStyle);
    state.follow_ai_offer(true);
    state.follow_ai_offer(false);
    render(&mut settings, &state);
    assert_eq!(
        settings.window.as_ref(),
        Some(&window),
        "the same window, drawn again"
    );
    let pages = settings.pages.clone().expect("the window's pages");
    assert!(pages.child_by_name(Category::WritingStyle.name()).is_none());
    assert_eq!(pages.visible_child_name().as_deref(), Some(first.name()));
    settings.close();
}
