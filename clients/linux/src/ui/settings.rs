//! The desktop Settings window, one page per category in `docs/settings.md`.

use std::sync::Arc;

use adw::prelude::*;
use mailcal_bindings::MailcalApp;
use widgets::{choice, dialog_box, group, group_titled, page_box};

use super::{
    AppInput,
    allodia_subscription::{SubscriptionInput, SubscriptionState},
    allodia_sync::AllodiaSyncState,
    icons,
};
use crate::{l10n, preferences::HostPreferences};

pub(super) mod about;
pub(super) mod account_sync_mode;
pub(super) mod accounts;
mod ai_endpoint;
pub(super) mod allodia;
pub(super) mod allodia_sync;
pub(super) mod subscription;
mod widgets;

mod diagnostics;
pub(super) mod general;
mod mcp;
mod pages;
pub(super) mod sender_name;
mod signature_editor;
pub(super) mod signatures;
mod state;
mod wizard;
mod writing_style;
mod writing_style_learn;
mod writing_style_learn_parts;
mod writing_style_reveal;
mod writing_style_reveal_cards;
mod writing_style_reveal_pages;

pub(super) use state::SettingsState;

/// A Settings category. Visible to the crate because a surface elsewhere can send the user to one
/// (the search horizon's route to the sync depth), and because [`super::AppInput`] carries it back
/// when the sidebar moves.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Category {
    Allodia,
    General,
    Calendar,
    Reading,
    Composing,
    Signatures,
    WritingStyle,
    Notifications,
    Privacy,
    Accounts,
    Advanced,
    Diagnostics,
    About,
}

#[derive(Clone, Copy)]
pub(super) struct RenderState<'a> {
    pub(super) generation: u64,
    pub(super) category: Category,
    pub(super) credential_repair_failed: Option<&'a str>,
    pub(super) allodia_signing_in: bool,
    pub(super) allodia_failure: Option<&'a str>,
    pub(super) allodia_sync: &'a AllodiaSyncState,
    /// What the subscription section has learned, for the same reason: a read and a write each
    /// outlive several window rebuilds.
    pub(super) allodia_subscription: &'a SubscriptionState,
    pub(super) allodia_accounts_synced:
        &'a std::collections::HashMap<String, mailcal_bindings::AllodiaAccountSyncMode>,
    /// This generation is a **refresh** of an already-open window, not a request to open one.
    ///
    /// The Allodia card changes by rebuilding the whole window, and its sign-in outlives whatever
    /// the user does next; so without this, a redirect landing after they closed Settings would
    /// put the window back on screen over their mail.
    pub(super) refresh_only: bool,
}

/// Every category, in order. What a given build **shows** is [`visible_categories`].
const CATEGORIES: [Category; 13] = [
    Category::Allodia,
    Category::General,
    Category::Calendar,
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
];

/// The categories this build shows.
///
/// [`Category::Allodia`] is absent when the build carries no registration, and the whole category
/// goes rather than its contents: a sidebar row that opens an empty page reads as a broken page
/// rather than as a build without a route, and that is what every build from source would see.
/// [`Category::WritingStyle`] likewise, while AI has nowhere to go (`writing_style`).
fn visible_categories(writing_style: bool) -> Vec<Category> {
    CATEGORIES
        .into_iter()
        .filter(|category| {
            *category != Category::Allodia || mailcal_bindings::allodia_sign_in_available()
        })
        .filter(|category| *category != Category::WritingStyle || writing_style)
        .collect()
}

#[derive(Clone)]
struct PageContext {
    app: Arc<MailcalApp>,
    preferences: Arc<HostPreferences>,
    sender: relm4::Sender<AppInput>,
    window: gtk::Window,
    navigation: gtk::Stack,
    credential_repair_failed: Option<String>,
    /// The Allodia sign-in's state. It lives in the model rather than in this window because the
    /// window is rebuilt from scratch on every change; a browser hop outlasts several rebuilds.
    allodia_signing_in: bool,
    allodia_failure: Option<String>,
    /// What the person's other devices have to say, for the same reason.
    allodia_sync: AllodiaSyncState,
    /// What the subscription section has learned, likewise.
    allodia_subscription: SubscriptionState,
    /// Where each account stands. Empty in a build with no Allodia sign-in, and the block is then
    /// absent rather than dead.
    allodia_accounts_synced:
        std::collections::HashMap<String, mailcal_bindings::AllodiaAccountSyncMode>,
    /// What a Writing style snapshot redraws in place.
    writing_style: writing_style::Live,
}

#[derive(Debug, Default)]
pub(super) struct SettingsWindow {
    window: Option<gtk::Window>,
    /// Kept beside the window because a refresh reuses both; the Done button lives here and must
    /// not be packed a second time.
    header: Option<adw::HeaderBar>,
    rendered_generation: u64,
    writing_style: writing_style::Live,
    /// The Writing style snapshot the window was last brought to.
    writing_style_seen: u64,
}

impl SettingsWindow {
    pub(super) fn render(
        &mut self,
        state: RenderState<'_>,
        parent: &adw::ApplicationWindow,
        app: Option<&Arc<MailcalApp>>,
        preferences: Arc<HostPreferences>,
        sender: relm4::Sender<AppInput>,
    ) {
        if state.generation == 0 || state.generation == self.rendered_generation {
            return;
        }
        if state.refresh_only && !self.is_on_screen() {
            return;
        }
        let Some(app) = app.cloned() else {
            return;
        };
        // A refresh redraws the window that is already on screen; it does not build a new one.
        //
        // Rebuilding would take the user's size and position with it, and `present()` would pull
        // focus back from wherever they are; which for the Allodia card is the **browser the
        // sign-in just opened**, one frame earlier. The card changing behind them must not steal
        // the window they are being asked to type into.
        let reuse = state.refresh_only && self.is_on_screen();
        let standing = reuse
            .then(|| self.window.clone().zip(self.header.clone()))
            .flatten();
        let (window, header) = if let Some(standing) = standing {
            standing
        } else {
            if let Some(window) = self.window.take() {
                window.close();
            }
            let (window, header) =
                crate::ui::modal::new(parent, l10n::settings_title(), 940, Some(680));
            window.set_modal(false);
            let done = gtk::Button::with_label(l10n::action_done());
            let closing = window.clone();
            done.connect_clicked(move |_| closing.close());
            header.pack_end(&done);
            // ⚠️ **Coming back to this window is the only signal a checkout gives.** The payment
            // finishes in a **browser** and this window stays open behind it, so without this the
            // person returns to the very card that sent them, still offering to sell what they
            // have just bought. The same staleness follows a subscription changing on a phone or
            // on the website, which this app is not present for either.
            //
            // Connected where the window is built rather than on every render: a refresh reuses
            // the window, and a handler added per refresh would fire once per redraw it has ever
            // had. The section ignores a return until it has answered once, so this costs a round
            // trip only on the page that draws one.
            let returning = sender.clone();
            window.connect_is_active_notify(move |window| {
                if window.is_active() {
                    returning.emit(AppInput::AllodiaSubscription(SubscriptionInput::Returned));
                }
            });
            (window, header)
        };
        let navigation = gtk::Stack::new();
        navigation.set_hexpand(true);
        navigation.set_vexpand(true);
        let ctx = PageContext {
            app,
            preferences,
            sender,
            window: window.clone(),
            navigation: navigation.clone(),
            credential_repair_failed: state.credential_repair_failed.map(str::to_owned),
            allodia_signing_in: state.allodia_signing_in,
            allodia_failure: state.allodia_failure.map(str::to_owned),
            allodia_sync: state.allodia_sync.clone(),
            allodia_subscription: state.allodia_subscription.clone(),
            allodia_accounts_synced: state.allodia_accounts_synced.clone(),
            writing_style: self.writing_style.clone(),
        };
        // Every page below registers what a snapshot redraws, afresh on each build.
        self.writing_style.clear();
        navigation.add_named(&window_content(state.category, &ctx), Some("settings"));
        navigation.set_visible_child_name("settings");
        window.set_child(Some(&navigation));
        if !reuse {
            window.present();
        }
        self.rendered_generation = state.generation;
        self.window = Some(window);
        self.header = Some(header);
    }

    pub(super) fn close(&mut self) {
        self.header = None;
        self.writing_style.clear();
        if let Some(window) = self.window.take() {
            window.close();
        }
    }

    /// Brings the open window to the Writing style snapshot the model last pulled, once per new
    /// one: the category and every credits line redraw in place.
    pub(super) fn sync_writing_style(&mut self, feed: &crate::ui::writing_style::WritingStyleFeed) {
        if feed.generation() == self.writing_style_seen {
            return;
        }
        self.writing_style_seen = feed.generation();
        if let Some(snapshot) = feed.snapshot() {
            self.writing_style.redraw(snapshot);
        }
    }

    /// Whether a Settings window is actually in front of the user.
    ///
    /// Deliberately not `self.window.is_some()`. The user closes this window through GTK: the
    /// Done button and the titlebar both call `close()` on the widget: which destroys it but
    /// leaves this handle holding it, so the `Option` stays `Some` for a window that is not on
    /// screen and never becomes `None` on the path a person actually takes. A destroyed widget
    /// does report itself as not visible, which is the question worth asking.
    fn is_on_screen(&self) -> bool {
        self.window.as_ref().is_some_and(WidgetExt::is_visible)
    }
}

fn window_content(category: Category, ctx: &PageContext) -> gtk::Box {
    if let Some(display) = gtk::gdk::Display::default() {
        icons::install(&display);
    }
    let shell = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let categories = gtk::ListBox::new();
    categories.add_css_class("navigation-sidebar");
    categories.set_selection_mode(gtk::SelectionMode::None);
    let stack = gtk::Stack::new();
    stack.set_hexpand(true);
    stack.set_vexpand(true);
    let shown = visible_categories(ctx.app.writing_styles().route.is_some());
    let selected = initial_category(&shown, category);
    let mut button_group: Option<gtk::ToggleButton> = None;
    for category in shown {
        let row = gtk::ListBoxRow::new();
        row.set_activatable(false);
        let button = category_button(category);
        if let Some(group) = &button_group {
            button.set_group(Some(group));
        } else {
            button_group = Some(button.clone());
        }
        let pages = stack.clone();
        let moved = ctx.sender.clone();
        button.connect_toggled(move |button| {
            if button.is_active() {
                pages.set_visible_child_name(category.name());
                // The stack switch is local to this window, so the model would otherwise never
                // learn where the person went; and the next redraw would take them somewhere else.
                moved.emit(AppInput::SettingsCategoryShown(category));
            }
        });
        row.set_child(Some(&button));
        categories.append(&row);
        stack.add_named(&page(category, ctx), Some(category.name()));
    }
    if let Some(button) = categories
        .row_at_index(selected)
        .and_then(|row| row.child())
        .and_downcast::<gtk::ToggleButton>()
    {
        button.set_active(true);
    }
    let sidebar = gtk::ScrolledWindow::new();
    sidebar.set_min_content_width(220);
    sidebar.set_child(Some(&categories));
    let split = gtk::Paned::new(gtk::Orientation::Horizontal);
    split.set_start_child(Some(&sidebar));
    split.set_end_child(Some(&stack));
    split.set_position(240);
    split.set_resize_start_child(false);
    split.set_shrink_start_child(false);
    split.set_vexpand(true);
    shell.append(&split);
    shell
}

/// One sidebar row's button: the category's glyph and its name, the way every other platform's
/// settings sidebar draws it (docs/settings.md).
///
/// The name is set on the button as well as drawn inside it, so assistive technology and the UI
/// suite find the row by the same words whatever the content widget does with them.
fn category_button(category: Category) -> gtk::ToggleButton {
    let content = adw::ButtonContent::new();
    content.set_icon_name(category.icon());
    content.set_label(category.title());
    content.set_halign(gtk::Align::Start);
    let button = gtk::ToggleButton::new();
    button.set_child(Some(&content));
    button.add_css_class("flat");
    button.set_halign(gtk::Align::Fill);
    button.set_hexpand(true);
    button.update_property(&[gtk::accessible::Property::Label(category.title())]);
    button
}

/// Which category the window opens on: the one the caller asked for, except when the screenshot
/// hook names `signatures`.
///
/// The hook wins over the request so a capture filed under `signatures` cannot silently show a
/// different category. Normal navigation uses the accessible toggle buttons in the sidebar.
///
/// `shown` is the sidebar's own list, never [`CATEGORIES`]: counted over every category instead, a
/// build carrying no registration selects the row *after* the one asked for, and the last category
/// selects no row at all.
fn initial_category(shown: &[Category], requested: Category) -> i32 {
    #[cfg(any(debug_assertions, feature = "dev-harness"))]
    if crate::showcase::screen() == Ok(crate::showcase::ShowcaseScreen::Signatures)
        && let Some(index) = index_of(shown, Category::Signatures)
    {
        return index;
    }
    index_of(shown, requested).unwrap_or_default()
}

fn index_of(shown: &[Category], wanted: Category) -> Option<i32> {
    shown
        .iter()
        .position(|category| *category == wanted)
        .and_then(|index| i32::try_from(index).ok())
}

fn page(category: Category, ctx: &PageContext) -> gtk::ScrolledWindow {
    let content = match category {
        Category::Allodia => allodia::page(ctx),
        Category::General => general::general(ctx),
        Category::Calendar => pages::calendar(ctx),
        Category::Reading => pages::reading(ctx),
        Category::Composing => pages::composing(ctx),
        Category::Signatures => signatures::signatures(ctx),
        Category::WritingStyle => writing_style::page(ctx),
        Category::Notifications => pages::notifications(ctx),
        Category::Privacy => pages::privacy(ctx),
        Category::Accounts => accounts::accounts(ctx),
        Category::Advanced => pages::advanced(ctx),
        Category::Diagnostics => diagnostics::diagnostics(ctx),
        Category::About => about::about(ctx),
    };
    let scroll = gtk::ScrolledWindow::new();
    scroll.set_child(Some(&content));
    scroll
}

impl Category {
    const fn name(self) -> &'static str {
        match self {
            Self::Allodia => "allodia",
            Self::General => "general",
            Self::Calendar => "calendar",
            Self::Reading => "reading",
            Self::Composing => "composing",
            Self::Signatures => "signatures",
            Self::WritingStyle => "writing-style",
            Self::Notifications => "notifications",
            Self::Privacy => "privacy",
            Self::Accounts => "accounts",
            Self::Advanced => "advanced",
            Self::Diagnostics => "diagnostics",
            Self::About => "about",
        }
    }

    fn title(self) -> &'static str {
        match self {
            // The category and the group inside it are the same thing named twice otherwise, and
            // the product's own name belongs in one string.
            Self::Allodia => l10n::settings_allodia_heading(),
            Self::General => l10n::settings_category_general(),
            Self::Calendar => l10n::settings_category_calendar(),
            Self::Reading => l10n::settings_category_reading(),
            Self::Composing => l10n::settings_category_composing(),
            Self::Signatures => l10n::settings_category_signatures(),
            Self::WritingStyle => l10n::settings_category_writing_style(),
            Self::Notifications => l10n::settings_category_notifications(),
            Self::Privacy => l10n::settings_category_privacy(),
            Self::Accounts => l10n::settings_category_accounts(),
            Self::Advanced => l10n::settings_category_advanced(),
            Self::Diagnostics => l10n::settings_category_diagnostics(),
            Self::About => l10n::settings_category_about(),
        }
    }

    const fn icon(self) -> &'static str {
        match self {
            Self::Allodia => icons::SETTINGS_ALLODIA,
            Self::General => icons::SETTINGS_GENERAL,
            Self::Calendar => icons::SETTINGS_CALENDAR,
            Self::Reading => icons::SETTINGS_READING,
            Self::Composing => icons::SETTINGS_COMPOSING,
            Self::Signatures => icons::SETTINGS_SIGNATURES,
            Self::WritingStyle => icons::SETTINGS_WRITING_STYLE,
            Self::Notifications => icons::SETTINGS_NOTIFICATIONS,
            Self::Privacy => icons::SETTINGS_PRIVACY,
            Self::Accounts => icons::SETTINGS_ACCOUNTS,
            Self::Advanced => icons::SETTINGS_ADVANCED,
            Self::Diagnostics => icons::SETTINGS_DIAGNOSTICS,
            Self::About => icons::SETTINGS_ABOUT,
        }
    }
}

#[cfg(test)]
#[path = "settings_tests.rs"]
pub(crate) mod tests;
