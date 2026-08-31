//! The display-timezone state machine: the active zone, any pending device-zone
//! change, and persistence.
//!
//! A display zone is a host app preference (`calendar-semantics.md`): the host detects
//! the OS zone natively and reports it; this adopts it on first boot, persists the
//! user's choice via [`mailcal_account`], and, when the device later reports a
//! *different* supported zone; raises a pending change the host prompts on. Every
//! adopted zone is validated against the engine ([`is_supported_zone`]) so an
//! unresolvable zone is never stored.

use std::path::PathBuf;

use engine_api::{Provider, TimeZoneId, is_supported_zone};
use mailcal_account::{load_preferences, save_preferences};
use mailcal_viewmodel::TimeZoneSnapshot;

use crate::{App, Surface};

/// Parses an IANA id into a [`TimeZoneId`] the bundled tzdb can resolve, or `None`.
pub(crate) fn supported_zone(id: &str) -> Option<TimeZoneId> {
    TimeZoneId::iana(id).ok().filter(is_supported_zone)
}

/// The active display zone, any pending device-zone change, and where to persist.
pub(crate) struct TimeZoneState {
    active: TimeZoneId,
    pending_device: Option<TimeZoneId>,
    prefs_path: Option<PathBuf>,
}

impl TimeZoneState {
    /// Builds the initial state from the host-reported `device` zone and the persisted
    /// preference at `prefs_path` (if any): on first boot (no stored zone) it adopts and
    /// **persists** the device zone; if a stored zone exists and the device differs, it
    /// starts with the stored zone and a pending change for the host to prompt on. An
    /// unsupported device zone falls back to UTC; an unsupported/absent stored zone is
    /// treated as first boot.
    pub(crate) fn new(device: TimeZoneId, prefs_path: Option<PathBuf>) -> Self {
        let device_supported = is_supported_zone(&device);
        let device = if device_supported {
            device
        } else {
            TimeZoneId::utc()
        };
        // First boot is a genuinely *absent* prefs file. A present-but-unreadable file
        // (a transient IO/permission hiccup, or a corrupt one) must NOT be treated as
        // first boot: doing so would overwrite the user's stored zone with the device
        // zone, silently losing their choice. So adoption is gated on the file's
        // absence, not merely on a failed read.
        let file_present = prefs_path.as_ref().is_some_and(|path| path.exists());
        let stored = prefs_path
            .as_ref()
            .and_then(|path| load_preferences(path).display_timezone)
            .and_then(|id| supported_zone(&id));
        let mut state = Self {
            active: stored.clone().unwrap_or_else(|| device.clone()),
            pending_device: None,
            prefs_path,
        };
        match stored {
            // First boot (no prefs file): adopt the device zone and persist it.
            None if !file_present => state.persist(),
            // Launched in a different *supported* device zone: prompt to update. An
            // unsupported device (coerced to UTC above) must not raise a spurious
            // "switch to Etc/UTC" prompt; matching how runtime reports ignore it.
            Some(stored) if device_supported && stored != device => {
                state.pending_device = Some(device);
            }
            // Otherwise keep the active zone untouched, without overwriting the file: a prefs
            // file that yielded no usable zone (unreadable/corrupt), or a device zone that
            // already matches or isn't supported.
            _ => {}
        }
        state
    }

    /// The active display zone (the order/localisation zone for the agenda).
    pub(crate) fn active(&self) -> TimeZoneId {
        self.active.clone()
    }

    /// The immutable view a host renders: the active zone and any pending change.
    pub(crate) fn snapshot(&self) -> TimeZoneSnapshot {
        TimeZoneSnapshot {
            active: self.active.as_str().to_owned(),
            pending_device: self
                .pending_device
                .as_ref()
                .map(|zone| zone.as_str().to_owned()),
        }
    }

    /// Records a device-reported zone. Returns `true` if the settings surface changed
    /// (a new pending change raised, or a stale one cleared because the device returned
    /// to the active zone). An unsupported zone is ignored. The active zone never
    /// changes here: only the user accepting does that.
    pub(crate) fn report_device(&mut self, device: TimeZoneId) -> bool {
        if !is_supported_zone(&device) {
            return false;
        }
        if device == self.active {
            self.pending_device.take().is_some()
        } else if self.pending_device.as_ref() == Some(&device) {
            false
        } else {
            self.pending_device = Some(device);
            true
        }
    }

    /// Adopts the pending device zone (the user accepted). Returns `true` if the active
    /// zone changed (so the caller re-orders the agenda).
    pub(crate) fn accept(&mut self) -> bool {
        match self.pending_device.take() {
            Some(zone) => {
                self.active = zone;
                self.persist();
                true
            }
            None => false,
        }
    }

    /// Dismisses the pending device zone (keep the current zone). Returns `true` if a
    /// pending change was cleared.
    pub(crate) fn dismiss(&mut self) -> bool {
        self.pending_device.take().is_some()
    }

    /// Sets the active zone explicitly via the selector. Returns `true` if the active
    /// zone changed. Always clears any pending change. An unsupported zone is ignored.
    pub(crate) fn set(&mut self, zone: TimeZoneId) -> bool {
        if !is_supported_zone(&zone) {
            return false;
        }
        self.pending_device = None;
        if zone == self.active {
            return false;
        }
        self.active = zone;
        self.persist();
        true
    }

    /// Persists the active zone to the prefs file (best effort; ignored if no path).
    ///
    /// Read-modify-write: the zone shares one file with account sync settings and other
    /// preferences, so this updates only the zone and preserves its siblings.
    fn persist(&self) {
        if let Some(path) = &self.prefs_path {
            let mut prefs = load_preferences(path);
            prefs.display_timezone = Some(self.active.as_str().to_owned());
            let _ = save_preferences(path, &prefs);
        }
    }
}

impl<P: Provider> App<P> {
    /// The active display zone: the agenda's ordering/localisation zone.
    pub(crate) fn active_zone(&self) -> TimeZoneId {
        self.timezone
            .lock()
            .expect("timezone mutex poisoned")
            .active()
    }

    /// Records a device-reported OS zone (the host pushes this on launch and on the
    /// OS's zone-change signal). Signals [`Surface::Settings`] when a pending change is
    /// raised or cleared; never changes the active zone (only the user accepting does).
    pub(crate) fn report_device_timezone(&self, id: &str) {
        let Some(zone) = supported_zone(id) else {
            return;
        };
        let raised = self
            .timezone
            .lock()
            .expect("timezone mutex poisoned")
            .report_device(zone);
        if raised {
            self.observer.surface_changed(Surface::Settings);
        }
    }

    /// Sets the active zone via the selector. Signals [`Surface::Settings`] and
    /// re-orders the agenda when the active zone actually changed.
    pub(crate) async fn set_timezone(&self, id: String) {
        let Some(zone) = supported_zone(&id) else {
            return;
        };
        let changed = self
            .timezone
            .lock()
            .expect("timezone mutex poisoned")
            .set(zone);
        self.observer.surface_changed(Surface::Settings);
        if changed {
            self.rebuild_calendar().await;
        }
    }

    /// Resolves a pending device-zone change: `accept` adopts it and re-orders the
    /// agenda, otherwise it is dismissed. Signals [`Surface::Settings`] on any change.
    pub(crate) async fn resolve_timezone_change(&self, accept: bool) {
        let changed = {
            let mut state = self.timezone.lock().expect("timezone mutex poisoned");
            if accept {
                state.accept()
            } else {
                state.dismiss()
            }
        };
        if accept {
            self.observer.surface_changed(Surface::Settings);
            if changed {
                self.rebuild_calendar().await;
            }
        } else if changed {
            self.observer.surface_changed(Surface::Settings);
        }
    }
}

#[cfg(test)]
#[path = "timezone_tests.rs"]
mod tests;

/// Display-timezone initialisation for [`App::new`](crate::App::new): the host-reported OS zone
/// and where to persist the user's chosen zone. Grouped so the constructor stays small.
#[derive(Debug, Clone)]
pub struct TimeZoneInit {
    /// The device's current OS timezone (an IANA id).
    pub device_zone: engine_api::TimeZoneId,
    /// Where the chosen display zone is persisted (`None` disables persistence: for the
    /// in-memory demo and tests).
    pub prefs_path: Option<std::path::PathBuf>,
}
