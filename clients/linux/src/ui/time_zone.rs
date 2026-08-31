//! Device-time-zone monitoring and the adopt-or-keep prompt.

use std::time::Duration;

use adw::prelude::*;
use mailcal_bindings::{MailcalApp, device_time_zone};

use super::AppInput;
use crate::{l10n, ui::modal};

#[derive(Debug)]
pub(super) struct DeviceZoneMonitor {
    current: String,
}

impl DeviceZoneMonitor {
    pub(super) const fn new(current: String) -> Self {
        Self { current }
    }

    pub(super) fn changed(&mut self, next: String) -> Option<String> {
        if next == self.current {
            return None;
        }
        self.current.clone_from(&next);
        Some(next)
    }
}

pub(super) fn device_zone() -> String {
    #[cfg(debug_assertions)]
    if let Ok(zone) = std::env::var("MAILCAL_FAKE_DEVICE_TIMEZONE")
        && !zone.is_empty()
    {
        return zone;
    }
    device_time_zone()
}

pub(super) fn poll_interval() -> Duration {
    #[cfg(debug_assertions)]
    if std::env::var("MAILCAL_FAKE_DEVICE_TIMEZONE").is_ok_and(|zone| !zone.is_empty()) {
        return Duration::from_secs(1);
    }
    Duration::from_mins(1)
}

#[derive(Debug, Default)]
pub(super) struct TimeZonePrompt {
    window: Option<gtk::Window>,
    pending: Option<String>,
}

impl TimeZonePrompt {
    pub(super) fn render(
        &mut self,
        app: Option<&MailcalApp>,
        parent: &adw::ApplicationWindow,
        sender: &relm4::Sender<AppInput>,
    ) {
        let snapshot = app.map(MailcalApp::timezone_settings);
        let pending = snapshot
            .as_ref()
            .and_then(|settings| settings.pending_device.clone());
        if pending == self.pending {
            return;
        }
        if let Some(window) = self.window.take() {
            window.destroy();
        }
        self.pending.clone_from(&pending);
        let (Some(pending), Some(settings)) = (pending, snapshot) else {
            return;
        };
        let (window, _) = modal::new(parent, l10n::tz_changed_title(), 460, None);
        let content = gtk::Box::new(gtk::Orientation::Vertical, 16);
        content.set_margin_start(24);
        content.set_margin_end(24);
        content.set_margin_top(24);
        content.set_margin_bottom(24);
        let message = gtk::Label::new(Some(&l10n::tz_changed_message(&pending)));
        message.set_wrap(true);
        message.set_xalign(0.0);
        content.append(&message);
        let actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        actions.set_halign(gtk::Align::End);
        let keep = gtk::Button::with_label(&l10n::tz_keep(&settings.active));
        let input = sender.clone();
        keep.connect_clicked(move |_| input.emit(AppInput::DismissDeviceTimeZone));
        actions.append(&keep);
        let update = gtk::Button::with_label(l10n::action_update());
        update.add_css_class("suggested-action");
        let input = sender.clone();
        update.connect_clicked(move |_| input.emit(AppInput::AcceptDeviceTimeZone));
        actions.append(&update);
        content.append(&actions);
        let input = sender.clone();
        window.connect_close_request(move |_| {
            input.emit(AppInput::DismissDeviceTimeZone);
            gtk::glib::Propagation::Proceed
        });
        window.set_child(Some(&content));
        window.present();
        self.window = Some(window);
    }
}

#[cfg(test)]
mod tests {
    use super::DeviceZoneMonitor;

    #[test]
    fn the_monitor_reports_only_a_real_device_zone_change() {
        let mut monitor = DeviceZoneMonitor::new("Europe/Amsterdam".to_owned());
        assert_eq!(monitor.changed("Europe/Amsterdam".to_owned()), None);
        assert_eq!(
            monitor.changed("America/New_York".to_owned()),
            Some("America/New_York".to_owned())
        );
        assert_eq!(monitor.changed("America/New_York".to_owned()), None);
    }
}
