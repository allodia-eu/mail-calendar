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
    (window, header)
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
}
