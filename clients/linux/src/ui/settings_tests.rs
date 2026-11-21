//! What [`super::SettingsState`] promises about opening, refreshing and staying put.
//!
//! Split from [`super`] to keep that file inside the 500-line limit. Pure state assertions: none
//! of this needs a window, which is why it is here rather than in the crate's single GTK test.

use adw::prelude::*;

use super::{
    CATEGORIES, Category, PaneComposer, Redraw, SettingsState, SettingsWindow,
    category::initial_category, visible_categories,
};

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
    assert_eq!(state.render_state().generation, 0, "nothing pending yet");

    state.open(Some(Category::Accounts));
    state.refresh_in_place();
    let refreshed = state.render_state();
    assert_eq!(refreshed.redraw, Redraw::InPlace);
    assert_eq!(refreshed.category, Category::Accounts);
    assert_eq!(refreshed.generation, 2);

    // The Settings button, which names no category: it must still open, and on the category
    // the refresh left behind.
    state.open(None);
    let opened = state.render_state();
    assert_eq!(
        opened.redraw,
        Redraw::Open,
        "a refresh must not outlive itself"
    );
    assert_eq!(opened.category, Category::Accounts);
    assert_eq!(opened.generation, 3);

    state.open(Some(Category::General));
    assert_eq!(state.render_state().category, Category::General);
}

/// Answering a question on one page must not move the person to another.
///
/// A redraw that named a page moved the person onto it: a change to the per-account sharing
/// control threw them onto the Allodia page mid-gesture, which reads as the app rejecting what
/// they just did. The Allodia card's signal redraws its page only while it is the one showing.
#[test]
fn a_change_made_on_a_page_redraws_it_rather_than_leaving_it() {
    let mut state = SettingsState::default();

    state.open(Some(Category::Accounts));
    state.refresh_in_place();
    let redrawn = state.render_state();
    assert_eq!(
        redrawn.category,
        Category::Accounts,
        "the page the change was made on is the page that redraws"
    );
    assert_eq!(redrawn.redraw, Redraw::InPlace, "a redraw, never an open");
    assert_eq!(redrawn.generation, 2, "and it does redraw");

    state.refresh_if_showing(Category::Allodia);
    let left = state.render_state();
    assert_eq!(
        left.category,
        Category::Accounts,
        "the Allodia signal moves nobody"
    );
    assert_eq!(
        left.generation, 2,
        "nor redraws a page that does not draw the card"
    );
}

/// The core's settings signal redraws Accounts, whose rows it describes, and no other page: the
/// rest reconcile themselves, and a rebuild under the signature editor would take what is being
/// written in it.
#[test]
fn the_cores_signal_redraws_the_accounts_page_and_no_other() {
    let mut state = SettingsState::default();

    state.open(Some(Category::Signatures));
    state.signalled();
    assert_eq!(state.render_state().generation, 1, "nothing to redraw");

    state.record_category(Category::Accounts);
    state.signalled();
    let signalled = state.render_state();
    assert_eq!(signalled.generation, 2);
    assert_eq!(
        signalled.redraw,
        Redraw::Signalled,
        "a redraw, never an open"
    );
    assert_eq!(signalled.category, Category::Accounts);
}

/// The window's checks that need a real `gtk::Window`. Called from the crate's single `gtk::init`
/// test.
pub(crate) fn the_window_is_on_screen_only_while_it_should_be() {
    a_closed_settings_window_is_not_on_screen();
    settings_stays_open_over_a_composer_already_in_the_pane();
}

/// The question the refresh guard has to ask, and the trap it sits in.
///
/// A user closes this window through GTK: the title bar's close button and Escape both call
/// `close()` on the widget, which destroys it but leaves the model's handle holding it. So
/// `is_some()` still answers yes on the only path a person actually takes, and a guard written on
/// it never fires: an Allodia redirect landing afterwards put Settings back over the user's mail.
fn a_closed_settings_window_is_not_on_screen() {
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

/// Settings' **Add account** opens setup over Settings, never over the main window behind it.
/// Both are children of the main window, so a setup window parented there is stacked beside
/// Settings rather than above it: coming back to the app raises Settings over it, and the button
/// does nothing, because setup is already open out of sight.
///
/// Called from the crate's single `gtk::init` test.
pub(crate) fn setup_opens_over_settings_while_settings_is_open() {
    use crate::ui::{AppInput, setup::SetupWindow, setup_state::SetupState};

    let root = gtk::Window::new();
    let mut settings = SettingsWindow::default();
    assert_eq!(
        settings.front_or(&root),
        root,
        "nothing is open over the app"
    );

    let window = gtk::Window::builder().transient_for(&root).build();
    window.present();
    settings.window = Some(window.clone());
    let front = settings.front_or(&root);
    assert_eq!(front, window, "Settings is in front");

    let (sender, _receiver) = relm4::channel::<AppInput>();
    let mut state = SetupState::closed();
    let mut setup = SetupWindow::default();
    state.open(false);
    setup.render(&state, &front, &sender);
    let open = setup.current_window().expect("setup is on screen");
    assert_eq!(open.transient_for().as_ref(), Some(&window));

    // Settings closing under it hands setup back to the main window, unchanged otherwise.
    window.close();
    setup.render(&state, &settings.front_or(&root), &sender);
    assert_eq!(
        setup.current_window().as_ref(),
        Some(&open),
        "the same window"
    );
    assert_eq!(open.transient_for().as_ref(), Some(&root));
    open.destroy();
    root.destroy();
}

#[test]
fn every_notice_is_raised_anew_and_redraws_the_page_in_place() {
    use super::notice::Notice;
    let mut state = SettingsState::default();
    state.open(Some(Category::Accounts));

    state.notify(Notice::Toast("Signed in again.".to_owned()));
    let first = state.render_state();
    assert_eq!(first.redraw, Redraw::InPlace, "a redraw, never an open");
    assert_eq!(first.generation, 2, "and it does redraw");
    let first = first.notice.map(|raised| raised.id);

    // The same words again are a second notice, so the window shows them a second time.
    state.notify(Notice::Toast("Signed in again.".to_owned()));
    let second = state.render_state().notice.map(|raised| raised.id);
    assert!(first.is_some() && second.is_some());
    assert_ne!(second, first);

    // A window opened afresh does not greet the person with one raised while it was closed,
    // and the next notice still carries a number the window has not shown.
    state.open(Some(Category::Accounts));
    assert!(state.render_state().notice.is_none());
    state.notify(Notice::Toast("Signed in again.".to_owned()));
    let third = state.render_state().notice.map(|raised| raised.id);
    assert!(third.is_some() && third != second && third != first);
}

#[test]
fn state_only_one_page_draws_redraws_that_page_and_no_other() {
    let mut state = SettingsState::default();
    state.open(Some(Category::Accounts));
    let opened = state.render_state().generation;

    // The subscription section is the Allodia page's: an account page is left alone, so a
    // dropdown open on it stays open.
    state.refresh_if_showing(Category::Allodia);
    assert_eq!(state.render_state().generation, opened);

    state.record_category(Category::Allodia);
    state.refresh_if_showing(Category::Allodia);
    let redrawn = state.render_state();
    assert!(redrawn.generation > opened);
    assert_eq!(redrawn.redraw, Redraw::InPlace);
}

/// Every page is built when the window opens, so a page hidden behind another category is drawn
/// before what it shows has arrived. A change it draws, landing while it is hidden, redraws it when
/// it is shown, or it shows the state it was built in: the subscription "checking" until the
/// window is left and come back to.
#[test]
fn a_page_that_changed_while_hidden_redraws_when_it_is_shown() {
    let mut state = SettingsState::default();
    state.open(Some(Category::General));
    let opened = state.render_state().generation;

    state.refresh_if_showing(Category::Allodia);
    assert_eq!(
        state.render_state().generation,
        opened,
        "hidden: nothing redraws"
    );

    state.record_category(Category::Allodia);
    let shown = state.render_state();
    assert!(shown.generation > opened, "shown: the page is drawn again");
    assert_eq!(shown.redraw, Redraw::InPlace, "a redraw, never an open");
    assert_eq!(shown.category, Category::Allodia);

    // Once: going back and forth after that is a switch of the stack, not a rebuild.
    let redrawn = shown.generation;
    state.record_category(Category::General);
    state.record_category(Category::Allodia);
    assert_eq!(state.render_state().generation, redrawn);

    // A whole window drawn since the change built the page fresh.
    state.record_category(Category::General);
    state.refresh_if_showing(Category::Allodia);
    state.open(None);
    let reopened = state.render_state().generation;
    state.record_category(Category::Allodia);
    assert_eq!(state.render_state().generation, reopened);
}

/// Settings opens over a composer and stays open: it is a window of its own, so opening it does
/// not leave the composer, and the composer keeps its draft by staying where it is
/// (`docs/drafts.md`). Only a composer *arriving*, from a mail link or a share, is put in front.
///
/// The regression: every render with a composer in the pane closed Settings, and opening Settings
/// is itself followed by a render (its sidebar reports the page it opened on), so the window went
/// as soon as it came.
fn settings_stays_open_over_a_composer_already_in_the_pane() {
    let mut settings = SettingsWindow::default();
    let window = gtk::Window::new();
    window.present();
    settings.window = Some(window);

    settings.give_way(PaneComposer::Shown, false);
    assert!(
        settings.is_on_screen(),
        "a composer already in the pane leaves Settings open"
    );
    settings.give_way(PaneComposer::Absent, false);
    assert!(settings.is_on_screen(), "and so does no composer at all");

    settings.give_way(PaneComposer::Arriving, false);
    assert!(
        !settings.is_on_screen(),
        "a composer arriving in the pane is put in front of Settings"
    );
}

#[test]
fn a_render_tells_a_composer_arriving_from_one_already_shown() {
    assert_eq!(PaneComposer::of(false, false), PaneComposer::Absent);
    assert_eq!(PaneComposer::of(false, true), PaneComposer::Absent);
    assert_eq!(PaneComposer::of(true, false), PaneComposer::Arriving);
    assert_eq!(PaneComposer::of(true, true), PaneComposer::Shown);
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

/// Writing style is in the sidebar exactly while the core offers its page, after Signatures
/// (`docs/settings.md`, `docs/ai.md` rule 4).
#[test]
fn writing_style_is_listed_only_while_offered_and_after_signatures() {
    assert!(!visible_categories(false).contains(&Category::WritingStyle));
    let offered = visible_categories(true);
    let at = offered
        .iter()
        .position(|category| *category == Category::WritingStyle)
        .expect("offered");
    assert_eq!(offered[at - 1], Category::Signatures);
}

/// A sidebar's icons are for finding a row without reading it, which two rows sharing one defeats.
/// The two account categories are the pair most easily given the same glyph.
#[test]
fn every_category_draws_its_own_glyph() {
    let glyphs = CATEGORIES.map(Category::icon);
    let distinct = glyphs.iter().collect::<std::collections::HashSet<_>>();
    assert_eq!(distinct.len(), glyphs.len(), "{glyphs:?}");
}

#[test]
fn taxonomy_order_matches_the_cross_platform_contract() {
    assert_eq!(
        CATEGORIES,
        [
            Category::Allodia,
            Category::General,
            Category::Calendar,
            Category::Videoconferencing,
            Category::Reading,
            Category::Composing,
            Category::Signatures,
            Category::WritingStyle,
            Category::Notifications,
            Category::Privacy,
            Category::Accounts,
            Category::Advanced,
            Category::Diagnostics,
            Category::About,
        ]
    );
}

/// What a sign-in's grant withheld opens the window on the account's page, where switching the
/// use on asks again, and says so over it.
#[test]
fn an_account_can_be_opened_with_something_to_say_about_it() {
    use super::notice::Notice;
    let mut state = SettingsState::default();
    state.open_on_account(
        "alice@example.com".to_owned(),
        Notice::Error {
            title: "Not everything you chose was allowed".to_owned(),
            detail: "Calendar".to_owned(),
        },
    );
    let opened = state.render_state();
    assert_eq!(opened.redraw, Redraw::Open);
    assert_eq!(state.account.as_deref(), Some("alice@example.com"));
    assert!(
        opened.notice.is_some(),
        "the notice survives the open it came with"
    );
}

/// A closed signature editor frees its view, and the web process each one runs with it. A handler
/// that holds the view, or the frame around it, keeps it alive.
///
/// Called from the crate's single `gtk::init` test.
pub(crate) fn a_closed_signature_editor_frees_its_view() {
    use std::{cell::RefCell, rc::Rc, sync::Arc};

    use mailcal_bindings::{LogLevel, Logger, MailcalApp};

    use super::{PageContext, signature_editor, signatures};
    use crate::{l10n, ui::setup_widget_tests::descendants};

    struct Quiet;
    impl Logger for Quiet {
        fn log(&self, _level: LogLevel, _target: String, _message: String) {}
    }

    let (sender, _receiver) = relm4::channel::<crate::ui::AppInput>();
    let app = MailcalApp::new_demo(
        Box::new(crate::observer::SurfaceObserver::new(sender.clone())),
        Box::new(Quiet),
        LogLevel::Error,
        "UTC".to_owned(),
    );
    let scratch = tempfile::tempdir().expect("a scratch directory");
    let window = gtk::Window::new();
    let navigation = gtk::Stack::new();
    navigation.add_named(
        &gtk::Box::new(gtk::Orientation::Vertical, 0),
        Some("settings"),
    );
    window.set_child(Some(&navigation));
    window.present();
    let ctx = PageContext {
        app: Arc::clone(&app),
        preferences: Arc::new(crate::preferences::HostPreferences::open(
            scratch.path().join("host.json"),
        )),
        sender,
        window: window.clone(),
        navigation: navigation.clone(),
        account: None,
        page: None,
        devices: None,
        signatures: signatures::Shown::default(),
        follow: super::calendar_follow::Followers::default(),
        ai_endpoint: super::calendar_follow::Followers::default(),
        writing_style: super::calendar_follow::Followers::default(),
        sidebar: super::category::Sidebar::default(),
    };

    let held: Rc<RefCell<Option<Rc<signature_editor::Editor>>>> = Rc::default();
    let closed = Rc::clone(&held);
    let editor = signature_editor::open(&ctx, &app.signatures_page(), None, move || {
        closed.replace(None);
    });
    held.replace(Some(editor));
    let shown = navigation
        .child_by_name("signature-editor")
        .expect("the editor");
    let view = descendants::<webkit6::WebView>(&shown)
        .into_iter()
        .next()
        .expect("the editor's view")
        .downgrade();
    descendants::<gtk::Button>(&shown)
        .into_iter()
        .find(|button| button.label().as_deref() == Some(l10n::action_cancel()))
        .expect("Cancel")
        .emit_clicked();
    drop(shown);

    // The editor leaves on the next turn of the loop, and the content filter's compile holds the
    // view until it answers.
    let context = gtk::glib::MainContext::default();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while view.upgrade().is_some() && std::time::Instant::now() < deadline {
        context.iteration(false);
    }
    assert!(held.borrow().is_none(), "Cancel closed the editor");
    assert!(
        view.upgrade().is_none(),
        "the closed signature editor's view is still alive"
    );
    window.destroy();
}
