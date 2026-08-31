//! Linux-host preferences that do not belong in the shared product core.

use std::{
    fs,
    path::PathBuf,
    sync::{Arc, Mutex, OnceLock},
};

use serde::{Deserialize, Serialize};

static PREFERENCES: OnceLock<Arc<HostPreferences>> = OnceLock::new();

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
struct Values {
    language: Option<String>,
    notifications_enabled: Option<bool>,
    diagnostics_debug: bool,
    /// The folder pane's width in pixels, or `None` for a pane nobody has dragged.
    folder_pane_width: Option<i32>,
}

/// Small, thread-safe preference store for settings owned by the GTK host.
#[derive(Debug)]
pub(crate) struct HostPreferences {
    path: PathBuf,
    values: Mutex<Values>,
}

impl HostPreferences {
    fn open(path: PathBuf) -> Self {
        let values = fs::read(&path)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default();
        Self {
            path,
            values: Mutex::new(values),
        }
    }

    pub(crate) fn language(&self) -> Option<String> {
        self.values
            .lock()
            .expect("host-preferences mutex poisoned")
            .language
            .clone()
    }

    pub(crate) fn set_language(&self, language: Option<&str>) {
        self.update(|values| {
            values.language = language.map(str::to_owned);
        });
    }

    pub(crate) fn notifications_enabled(&self) -> bool {
        self.values
            .lock()
            .expect("host-preferences mutex poisoned")
            .notifications_enabled
            .unwrap_or(true)
    }

    pub(crate) fn set_notifications_enabled(&self, enabled: bool) {
        self.update(|values| values.notifications_enabled = Some(enabled));
    }

    /// The folder pane's stored width, or `None` for a pane nobody has dragged. Both the default
    /// and the clamp belong to the pane, not here: what a width may be depends on how much room
    /// there is to divide, which this store cannot know.
    pub(crate) fn folder_pane_width(&self) -> Option<i32> {
        self.values
            .lock()
            .expect("host-preferences mutex poisoned")
            .folder_pane_width
    }

    pub(crate) fn set_folder_pane_width(&self, width: i32) {
        self.update(|values| values.folder_pane_width = Some(width));
    }

    pub(crate) fn diagnostics_debug(&self) -> bool {
        self.values
            .lock()
            .expect("host-preferences mutex poisoned")
            .diagnostics_debug
    }

    pub(crate) fn set_diagnostics_debug(&self, enabled: bool) {
        self.update(|values| values.diagnostics_debug = enabled);
    }

    fn update(&self, change: impl FnOnce(&mut Values)) {
        let mut values = self.values.lock().expect("host-preferences mutex poisoned");
        change(&mut values);
        self.persist(&values);
    }

    fn persist(&self, values: &Values) {
        let Some(parent) = self.path.parent() else {
            return;
        };
        if fs::create_dir_all(parent).is_err() {
            return;
        }
        let Ok(bytes) = serde_json::to_vec_pretty(values) else {
            return;
        };
        let temporary = self.path.with_extension("json.tmp");
        if fs::write(&temporary, bytes).is_ok() {
            let _ = fs::rename(temporary, &self.path);
        }
    }
}

pub(crate) fn global() -> Arc<HostPreferences> {
    Arc::clone(PREFERENCES.get_or_init(|| {
        Arc::new(HostPreferences::open(
            gtk::glib::user_config_dir().join("mailcal/host.json"),
        ))
    }))
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::HostPreferences;

    fn scratch() -> std::path::PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock after epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("mailcal-linux-preferences-{nonce}.json"))
    }

    #[test]
    fn absent_notification_choice_defaults_on() {
        let preferences = HostPreferences::open(scratch());

        assert!(preferences.notifications_enabled());
        assert!(!preferences.diagnostics_debug());
        assert_eq!(preferences.language(), None);
        assert_eq!(preferences.folder_pane_width(), None);
    }

    #[test]
    fn host_choices_survive_reopening() {
        let path = scratch();
        let preferences = HostPreferences::open(path.clone());
        preferences.set_language(Some("nl"));
        preferences.set_notifications_enabled(false);
        preferences.set_diagnostics_debug(true);
        preferences.set_folder_pane_width(320);

        let reopened = HostPreferences::open(path);
        assert_eq!(reopened.language().as_deref(), Some("nl"));
        assert!(!reopened.notifications_enabled());
        assert!(reopened.diagnostics_debug());
        assert_eq!(reopened.folder_pane_width(), Some(320));
    }
}
