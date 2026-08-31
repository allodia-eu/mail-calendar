//! What [`super::SettingsState`] promises about opening, refreshing and staying put.
//!
//! Split from [`super`] to keep that file inside the 500-line limit. Pure state assertions: none
//! of this needs a window, which is why it is here rather than in the crate's single GTK test.

use adw::prelude::*;

use super::{CATEGORIES, Category, SettingsState, SettingsWindow, initial_category};

/// A generation is either a request to **open** the window or a redraw of an open one, and the
/// two must not leak into each other.
///
/// The regression: `refresh_only` was a field beside the generation, and two of the four sites
/// that bumped the generation forgot to clear it: so after any Allodia sign-in a stale `true`
/// sat there and the *next* open (the horizon line's route to the sync depth, or a repaired
/// credential) silently did nothing at all.
#[test]
fn an_open_is_never_left_holding_a_previous_refreshs_intent() {
    let mut state = SettingsState::default();
    let synced = std::collections::HashMap::new();
    assert_eq!(
        state.render_state(None, &synced).generation,
        0,
        "nothing pending yet"
    );

    state.refresh(Category::Accounts);
    let refreshed = state.render_state(None, &synced);
    assert!(refreshed.refresh_only);
    assert_eq!(refreshed.category, Category::Accounts);
    assert_eq!(refreshed.generation, 1);

    // The Settings button, which names no category: it must still open, and on the category
    // the refresh left behind.
    state.open(None);
    let opened = state.render_state(None, &synced);
    assert!(!opened.refresh_only, "a refresh must not outlive itself");
    assert_eq!(opened.category, Category::Accounts);
    assert_eq!(opened.generation, 2);

    state.open(Some(Category::General));
    assert_eq!(
        state.render_state(None, &synced).category,
        Category::General
    );
}

/// Answering a question on one page must not move the person to another.
///
/// The regression: every Allodia redraw named `Category::Allodia`, including the ones a change
/// on **Accounts** triggers. Moving the per-account sharing control therefore threw the person
/// onto the Allodia page mid-gesture; which reads as the app rejecting what they just did,
/// and hides whether it took effect. A browser hop coming back still names Allodia, because
/// that is the page it was started from; a change made in place does not.
#[test]
fn a_change_made_on_a_page_redraws_it_rather_than_leaving_it() {
    let mut state = SettingsState::default();
    let synced = std::collections::HashMap::new();

    state.open(Some(Category::Accounts));
    state.refresh_in_place();
    let redrawn = state.render_state(None, &synced);
    assert_eq!(
        redrawn.category,
        Category::Accounts,
        "the page the change was made on is the page that redraws"
    );
    assert!(redrawn.refresh_only, "a redraw, never an open");
    assert_eq!(redrawn.generation, 2, "and it does redraw");

    // The contrast, which is the behaviour worth keeping: a hop coming back names its page.
    state.refresh(Category::Allodia);
    assert_eq!(
        state.render_state(None, &synced).category,
        Category::Allodia
    );
}

/// The question the refresh guard has to ask, and the trap it sits in.
///
/// A user closes this window through GTK: the Done button and the titlebar both call `close()`
/// on the widget: which destroys it but leaves the model's handle holding it. So `is_some()`
/// still answers yes on the only path a person actually takes, and a guard written on it never
/// fires: an Allodia redirect landing afterwards put Settings back over the user's mail.
///
/// Called from the crate's single `gtk::init` test.
pub(crate) fn a_closed_settings_window_is_not_on_screen() {
    let mut settings = SettingsWindow::default();
    assert!(!settings.is_on_screen(), "nothing has been opened yet");

    let window = gtk::Window::new();
    window.present();
    settings.window = Some(window.clone());
    assert!(settings.is_on_screen());

    window.close();
    assert!(
        settings.window.is_some(),
        "the handle outlives the window, which is why the guard may not ask `is_some`"
    );
    assert!(
        !settings.is_on_screen(),
        "a refresh landing now must leave the user's mail alone"
    );
}

/// The sidebar selection counts rows the sidebar actually has.
///
/// A build carrying no Allodia registration draws one row fewer. Counted over every category,
/// every request lands one row late; Settings → Signatures opens Notifications; and About,
/// last in the list, matches no row at all and leaves the window with nothing selected.
#[test]
fn a_build_without_the_allodia_route_still_opens_the_category_asked_for() {
    let without: Vec<Category> = CATEGORIES
        .into_iter()
        .filter(|category| *category != Category::Allodia)
        .collect();

    for (index, category) in without.iter().enumerate() {
        assert_eq!(
            initial_category(&without, *category),
            i32::try_from(index).expect("a category index fits an i32"),
            "{category:?} must select its own row"
        );
    }
    assert_eq!(
        initial_category(&CATEGORIES, Category::About),
        i32::try_from(CATEGORIES.len() - 1).expect("a category index fits an i32"),
        "and the full list is unchanged"
    );
}

#[test]
fn taxonomy_order_matches_the_cross_platform_contract() {
    assert_eq!(
        CATEGORIES,
        [
            Category::Allodia,
            Category::General,
            Category::Calendar,
            Category::Reading,
            Category::Composing,
            Category::Signatures,
            Category::Notifications,
            Category::Privacy,
            Category::Accounts,
            Category::Advanced,
            Category::Diagnostics,
            Category::About,
        ]
    );
}
