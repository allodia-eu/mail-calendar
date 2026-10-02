//! Shared chrome for transient Linux windows.

use adw::prelude::*;

/// Builds a modal whose title is rendered by its client-side title bar exactly once.
pub(super) fn new(
    parent: &impl IsA<gtk::Window>,
    title: &str,
    width: i32,
    height: Option<i32>,
) -> (gtk::Window, adw::HeaderBar) {
    let window = gtk::Window::builder()
        .title(title)
        .transient_for(parent)
        .modal(true)
        .default_width(width)
        .build();
    if let Some(height) = height {
        window.set_default_height(height);
    }
    let header = adw::HeaderBar::new();
    header.set_title_widget(Some(&adw::WindowTitle::new(title, "")));
    window.set_titlebar(Some(&header));
    window.add_controller(escape_closes());
    (window, header)
}

/// Escape closes a modal as its close button does: through `close-request`, so a modal that keeps
/// something on close keeps it here too, and one that refuses to close refuses this as well. A
/// plain `GtkWindow` binds no key to closing. Bubble phase, so a field or popover inside that uses
/// Escape itself gets it first.
fn escape_closes() -> gtk::ShortcutController {
    let keys = gtk::ShortcutController::new();
    keys.add_shortcut(gtk::Shortcut::new(
        gtk::ShortcutTrigger::parse_string("Escape"),
        Some(gtk::NamedAction::new("window.close")),
    ));
    keys
}

#[cfg(test)]
pub(crate) mod tests {
    use adw::prelude::*;

    pub(crate) fn a_modal_renders_its_title_in_native_chrome_only() {
        let parent = gtk::Window::new();
        let (window, header) = super::new(&parent, "One title", 420, Some(240));
        let body = gtk::Label::new(Some("Dialog body"));
        window.set_child(Some(&body));

        assert_eq!(window.title().as_deref(), Some("One title"));
        assert_eq!(
            window.titlebar().as_ref(),
            Some(header.upcast_ref::<gtk::Widget>())
        );
        assert_eq!(
            window
                .child()
                .and_downcast::<gtk::Label>()
                .expect("modal body")
                .text(),
            "Dialog body"
        );
    }

    pub(crate) fn escape_closes_a_modal_as_its_close_button_does() {
        let parent = gtk::Window::new();
        let (window, _) = super::new(&parent, "Closes on Escape", 420, None);
        let bindings: Vec<(String, String)> = window
            .observe_controllers()
            .into_iter()
            .filter_map(|controller| controller.ok()?.downcast::<gtk::ShortcutController>().ok())
            .flat_map(|keys| {
                keys.iter::<gtk::glib::Object>()
                    .filter_map(|item| item.ok()?.downcast::<gtk::Shortcut>().ok())
                    .collect::<Vec<_>>()
            })
            .filter_map(|shortcut| {
                Some((
                    shortcut.trigger()?.to_str().to_string(),
                    shortcut.action()?.to_str().to_string(),
                ))
            })
            .collect();
        assert!(
            bindings.contains(&("Escape".to_owned(), "action(window.close)".to_owned())),
            "{bindings:?}"
        );
    }
}
