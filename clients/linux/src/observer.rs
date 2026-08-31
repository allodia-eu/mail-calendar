//! Marshals core surface notifications onto the Relm4/GLib event loop.

use mailcal_bindings::{Observer, Surface};

use crate::ui::AppInput;

/// Cheap callback bridge: Relm4's thread-safe input channel wakes the component on the GLib loop.
#[derive(Debug)]
pub(crate) struct SurfaceObserver {
    sender: relm4::Sender<AppInput>,
}

impl SurfaceObserver {
    pub(crate) fn new(sender: relm4::Sender<AppInput>) -> Self {
        Self { sender }
    }
}

impl Observer for SurfaceObserver {
    fn surface_changed(&self, surface: Surface) {
        self.sender.emit(AppInput::SurfaceChanged(surface));
    }
}

#[cfg(test)]
mod tests {
    use mailcal_bindings::{Observer, Surface};

    use super::SurfaceObserver;
    use crate::ui::AppInput;

    #[test]
    fn callback_crosses_the_relm_channel() {
        let (sender, receiver) = relm4::channel();
        let observer = SurfaceObserver::new(sender);

        observer.surface_changed(Surface::MailboxList);

        assert!(matches!(
            receiver.recv_sync(),
            Some(AppInput::SurfaceChanged(Surface::MailboxList))
        ));
    }
}
