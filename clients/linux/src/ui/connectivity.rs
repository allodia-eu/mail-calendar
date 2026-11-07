//! OS reachability, reported to the core. What the core makes of it (offline, an account it
//! cannot reach) comes back as notices (`docs/notices.md`).

use gtk::gio::{self, prelude::NetworkMonitorExt};
use mailcal_bindings::{Intent, MailcalApp};

use super::AppInput;

/// Reports reachability before the first GTK render, and keeps reporting it.
pub(super) fn at_launch(
    app: Option<&MailcalApp>,
    sender: &relm4::Sender<AppInput>,
) -> gio::NetworkMonitor {
    let monitor = observe_network(sender);
    let reachable = monitor.is_network_available();
    if let Some(app) = app {
        app.dispatch(Intent::ReportNetworkReachable { reachable });
        // The cached snapshot is already ready to paint while offline. Coming online dispatches
        // its own catch-up refresh, so only a reachable launch needs this first one.
        if reachable {
            app.dispatch(Intent::RefreshMail);
        }
    }
    monitor
}

/// Retains GIO's default-network subscription and forwards changes onto the GLib/Relm loop.
pub(super) fn observe_network(sender: &relm4::Sender<AppInput>) -> gio::NetworkMonitor {
    let monitor = gio::NetworkMonitor::default();
    let input = sender.clone();
    monitor.connect_network_changed(move |_, reachable| {
        report_reachability(&input, reachable);
    });
    monitor
}

fn report_reachability(sender: &relm4::Sender<AppInput>, reachable: bool) {
    sender.emit(AppInput::NetworkReachabilityChanged(reachable));
}

#[cfg(test)]
mod tests {
    use super::report_reachability;
    use crate::ui::AppInput;

    #[test]
    fn gio_reachability_crosses_the_relm_channel_with_its_value() {
        let (sender, receiver) = relm4::channel();

        report_reachability(&sender, false);

        assert!(matches!(
            receiver.recv_sync(),
            Some(AppInput::NetworkReachabilityChanged(false))
        ));
    }
}
