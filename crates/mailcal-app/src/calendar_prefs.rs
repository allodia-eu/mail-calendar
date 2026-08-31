//! What the user has decided about each calendar: whether to draw it, and in what colour.
//!
//! This is the state behind the calendar manager ("Agenda's beheren"). It is applied at **read**
//! time, in [`crate::calendar_cache`]'s page query, rather than baked into the cache: so toggling
//! a calendar off redraws the grid immediately, with no re-sync and no network. The cache holds
//! what the *server* said; this holds what the *user* said.
//!
//! Keyed on account **and** calendar id. A calendar id is unique only within its account, so two
//! accounts can each have a `work` calendar, and the previous in-memory set, keyed on the id
//! alone, would have hidden both when the user hid one.

use std::path::PathBuf;

use engine_api::Provider;
use mailcal_account::{
    CalendarPrefs, DefaultCalendar, Preferences, load_preferences, save_preferences,
};

use crate::App;

/// The loaded per-calendar preferences and where to persist them.
pub(crate) struct CalendarPrefsState {
    prefs: Preferences,
    prefs_path: Option<PathBuf>,
}

impl CalendarPrefsState {
    /// Loads the persisted decisions (none, i.e. every calendar visible in its server colour, when
    /// the file is absent or unreadable).
    pub(crate) fn new(prefs_path: Option<PathBuf>) -> Self {
        let prefs = prefs_path
            .as_ref()
            .map_or_else(Preferences::default, load_preferences);
        Self { prefs, prefs_path }
    }

    /// What the user decided about one calendar: the default for one they never touched.
    pub(crate) fn get(&self, account: &str, calendar: &str) -> CalendarPrefs {
        self.prefs.calendar(account, calendar)
    }

    /// The calendar the user chose for new events, or `None` if they never chose one.
    ///
    /// The *stored* choice, which may name a calendar that has since gone or turned read-only;
    /// resolving that is [`crate::calendar_colors`]'s job, where the calendar list is in hand.
    pub(crate) fn default_calendar(&self) -> Option<&DefaultCalendar> {
        self.prefs.default_calendar.as_ref()
    }

    /// Records which calendar new events go to, and persists it.
    fn set_default_calendar(&mut self, choice: Option<DefaultCalendar>) {
        self.prefs.default_calendar = choice;
        self.persist();
    }

    /// Records a decision and persists it, preserving every sibling preference in the file.
    fn set(&mut self, account: &str, calendar: &str, prefs: CalendarPrefs) {
        self.prefs.set_calendar(account, calendar, prefs);
        self.persist();
    }

    /// Drops every calendar decision for an account, and persists: so removing an account leaves
    /// no stale colour or visibility for a later re-add to inherit (which is otherwise how a
    /// re-added account keeps a colour the user thought they had cleared by removing it).
    pub(crate) fn remove_account(&mut self, account: &str) {
        let named_it = self
            .prefs
            .default_calendar
            .as_ref()
            .is_some_and(|choice| choice.account == account);
        if named_it {
            // A choice pointing into an account that is gone would otherwise sit in the file and
            // come back to life if the account were re-added: the same staleness the calendar
            // decisions above are cleared to avoid.
            self.prefs.default_calendar = None;
        }
        if self.prefs.remove_account_calendars(account) || named_it {
            self.persist();
        }
    }

    /// Writes the current calendar decisions back, read-modify-write against what is on disk right
    /// now, so a concurrent write to an unrelated preference is not clobbered by our in-memory
    /// copy.
    fn persist(&self) {
        if let Some(path) = &self.prefs_path {
            let mut on_disk = load_preferences(path);
            on_disk.calendars = self.prefs.calendars.clone();
            on_disk
                .default_calendar
                .clone_from(&self.prefs.default_calendar);
            let _ = save_preferences(path, &on_disk);
        }
    }
}

impl<P: Provider> App<P> {
    /// Shows or hides one calendar's events, and persists the choice.
    ///
    /// Takes effect on the next page pull: the grid filters at read time, so this needs no sync
    /// and no network, and an unticked calendar disappears at once.
    // `async` with no inner `await` is intentional: every dispatched command method shares one
    // async shape so `dispatch` and the FFI adapter drive them uniformly.
    #[allow(clippy::unused_async)]
    pub async fn set_calendar_visible(&self, account: &str, calendar: &str, visible: bool) {
        self.edit_calendar(account, calendar, |prefs| prefs.visible = visible);
    }

    /// Overrides one calendar's colour (or clears the override, back to the server's colour).
    ///
    /// The hex is **not** trusted: it is snapped to the nearest palette entry by the colour
    /// resolver on the way out, so a client cannot introduce an off-palette colour; including
    /// Allodia Orange, which is reserved for actions.
    #[allow(clippy::unused_async)]
    pub async fn set_calendar_color(&self, account: &str, calendar: &str, hex: Option<String>) {
        self.edit_calendar(account, calendar, |prefs| prefs.color = hex);
    }

    /// Chooses the calendar new events are filed on, and persists it.
    ///
    /// `None` clears the choice, which is not the same as picking one: it returns to "whichever
    /// writable calendar comes first", so an account added later can become the default on its own.
    #[allow(clippy::unused_async)]
    pub async fn set_default_calendar(&self, choice: Option<DefaultCalendar>) {
        self.calendar_prefs
            .lock()
            .expect("calendar-prefs mutex poisoned")
            .set_default_calendar(choice);
        // Nothing about the *events* changed, but the calendar list carries which row is the
        // default, and that list rides along with the page.
        self.calendar.resignal();
    }

    /// Applies an edit to one calendar's preferences, persists it, and tells the host the grid is
    /// stale.
    fn edit_calendar(&self, account: &str, calendar: &str, edit: impl FnOnce(&mut CalendarPrefs)) {
        let mut state = self
            .calendar_prefs
            .lock()
            .expect("calendar-prefs mutex poisoned");
        let mut prefs = state.get(account, calendar);
        edit(&mut prefs);
        state.set(account, calendar, prefs);
        drop(state);
        // As in `display_settings`: preferences are applied when the host pulls a page, so this
        // announces staleness rather than a value.
        self.calendar.resignal();
    }
}

#[cfg(test)]
mod tests {
    use mailcal_account::{QuoteStyle, load_preferences, save_preferences};

    use super::CalendarPrefsState;

    #[test]
    fn a_calendar_nobody_touched_is_visible_in_its_server_colour() {
        // The default matters more than it looks: `bool::default()` is `false`, so getting this
        // wrong would hide every calendar the user has never opened the manager for, all of them.
        let prefs = CalendarPrefsState::new(None).get("acct-1", "work");
        assert!(prefs.visible);
        assert_eq!(prefs.color, None);
    }

    #[test]
    fn two_accounts_can_each_have_a_work_calendar() {
        // The bug this keying exists to prevent: a calendar id is unique only WITHIN its account,
        // so a flat map would let hiding one account's `work` hide the other's too;
        // silently, and only for the user who happens to have two.
        let dir = std::env::temp_dir().join("mailcal-calendar-prefs-scope-test");
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("preferences.toml");
        save_preferences(&path, &mailcal_account::Preferences::default()).unwrap();

        let mut state = CalendarPrefsState::new(Some(path));
        let mut hidden = state.get("acct-1", "work");
        hidden.visible = false;
        state.set("acct-1", "work", hidden);

        assert!(!state.get("acct-1", "work").visible);
        assert!(
            state.get("acct-2", "work").visible,
            "the other account's calendar of the same name must be untouched"
        );
    }

    #[test]
    fn removing_an_account_clears_its_calendar_decisions() {
        // The persisted-colour bug: a colour override (and a hidden calendar) survived removing the
        // account, so a re-add inherited it instead of the fresh distinct-hue default. Removal must
        // clear the account's decisions, and leave another account's alone.
        let dir = std::env::temp_dir().join("mailcal-calendar-prefs-remove-test");
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("preferences.toml");
        save_preferences(&path, &mailcal_account::Preferences::default()).unwrap();

        let mut state = CalendarPrefsState::new(Some(path.clone()));
        let mut gone = state.get("gone", "work");
        gone.visible = false;
        gone.color = Some("#3f8f55".to_owned());
        state.set("gone", "work", gone);
        let mut kept = state.get("kept", "work");
        kept.color = Some("#2f6fa8".to_owned());
        state.set("kept", "work", kept);

        state.remove_account("gone");

        // Back to the defaults for the removed account; in memory and on disk.
        let after = state.get("gone", "work");
        assert!(after.visible);
        assert_eq!(after.color, None);
        assert_eq!(
            CalendarPrefsState::new(Some(path))
                .get("gone", "work")
                .color,
            None
        );
        // The other account is untouched.
        assert_eq!(state.get("kept", "work").color.as_deref(), Some("#2f6fa8"));
    }

    #[test]
    fn decisions_persist_and_reload_without_clobbering_siblings() {
        let dir = std::env::temp_dir().join("mailcal-calendar-prefs-test");
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("preferences.toml");
        let seeded = mailcal_account::Preferences {
            quote_style: QuoteStyle::LineAndHeader,
            ..Default::default()
        };
        save_preferences(&path, &seeded).unwrap();

        let mut state = CalendarPrefsState::new(Some(path.clone()));
        let mut prefs = state.get("acct-1", "work");
        prefs.visible = false;
        prefs.color = Some("#3f8f55".to_owned());
        state.set("acct-1", "work", prefs);

        let reloaded = CalendarPrefsState::new(Some(path.clone()));
        let back = reloaded.get("acct-1", "work");
        assert!(!back.visible);
        assert_eq!(back.color.as_deref(), Some("#3f8f55"));
        assert_eq!(
            load_preferences(&path).quote_style,
            QuoteStyle::LineAndHeader
        );
    }
}
