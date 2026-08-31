//! The standing question for a sent message whose copy could not be filed.

use std::cell::RefCell;

use adw::prelude::*;

use super::AppInput;
use crate::l10n;

#[derive(Debug)]
pub(super) struct UnfiledCopyNotice {
    pub(super) body: String,
    pub(super) retrying: bool,
}

/// A modal that stays open until the core clears the question.
pub(super) struct UnfiledCopyPrompt {
    widgets: RefCell<Option<PromptWidgets>>,
    sender: relm4::Sender<AppInput>,
}

struct PromptWidgets {
    window: gtk::Window,
    body: gtk::Label,
    retry: gtk::Button,
    dismiss: gtk::Button,
}

impl UnfiledCopyPrompt {
    pub(super) fn new(sender: &relm4::Sender<AppInput>) -> Self {
        Self {
            widgets: RefCell::new(None),
            sender: sender.clone(),
        }
    }

    fn build_widgets(&self, parent: &adw::ApplicationWindow) -> PromptWidgets {
        let (window, _) = crate::ui::modal::new(parent, l10n::unfiled_copy_title(), 460, None);
        window.set_deletable(false);
        window.connect_close_request(|_| gtk::glib::Propagation::Stop);

        let shell = gtk::Box::new(gtk::Orientation::Vertical, 12);
        shell.set_margin_start(24);
        shell.set_margin_end(24);
        shell.set_margin_top(24);
        shell.set_margin_bottom(24);

        let body = gtk::Label::new(None);
        body.set_use_markup(false);
        body.set_xalign(0.0);
        body.set_wrap(true);
        body.set_wrap_mode(gtk::pango::WrapMode::WordChar);
        shell.append(&body);

        let actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        actions.set_halign(gtk::Align::End);
        actions.set_margin_top(6);
        let dismiss = gtk::Button::with_label(l10n::unfiled_copy_dismiss());
        let input = self.sender.clone();
        dismiss.connect_clicked(move |_| input.emit(AppInput::DismissUnfiledCopy));
        actions.append(&dismiss);
        let retry = gtk::Button::with_label(l10n::unfiled_copy_retry());
        retry.add_css_class("suggested-action");
        let input = self.sender.clone();
        retry.connect_clicked(move |_| input.emit(AppInput::RetryUnfiledCopy));
        actions.append(&retry);
        shell.append(&actions);
        window.set_child(Some(&shell));

        PromptWidgets {
            window,
            body,
            retry,
            dismiss,
        }
    }

    pub(super) fn render(
        &self,
        notice: Option<&UnfiledCopyNotice>,
        parent: &adw::ApplicationWindow,
    ) {
        let Some(notice) = notice else {
            if let Some(widgets) = self.widgets.borrow().as_ref() {
                widgets.window.set_visible(false);
            }
            return;
        };
        let mut widgets = self.widgets.borrow_mut();
        let widgets = widgets.get_or_insert_with(|| self.build_widgets(parent));
        widgets.body.set_text(&notice.body);
        widgets.retry.set_sensitive(!notice.retrying);
        widgets.dismiss.set_sensitive(!notice.retrying);
        if !widgets.window.is_visible() {
            widgets.window.present();
        }
    }

    #[cfg(test)]
    fn test_widgets(&self) -> (gtk::Window, gtk::Label, gtk::Button, gtk::Button) {
        let widgets = self.widgets.borrow();
        let widgets = widgets.as_ref().expect("prompt widgets");
        (
            widgets.window.clone(),
            widgets.body.clone(),
            widgets.retry.clone(),
            widgets.dismiss.clone(),
        )
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use adw::prelude::*;

    use super::{UnfiledCopyNotice, UnfiledCopyPrompt};
    use crate::ui::AppInput;

    pub(crate) fn the_unfiled_copy_question_offers_both_answers_and_blocks_double_answers() {
        let (sender, receiver) = relm4::channel();
        let prompt = UnfiledCopyPrompt::new(&sender);
        let notice = UnfiledCopyNotice {
            body: "The sent copy is missing".to_owned(),
            retrying: false,
        };

        // `NON_UNIQUE`, so registering never negotiates for the application id. A widget test
        // is not a singleton app, and an owner already on the session bus; an abandoned run of
        // this test still holding the name; otherwise makes this the *remote* instance, which
        // then blocks on `org.gtk.Actions.DescribeAll()` against a process that answers nothing.
        let application = adw::Application::builder()
            .application_id(format!("{}.unfiled-test", crate::l10n::APP_ID))
            .flags(gtk::gio::ApplicationFlags::NON_UNIQUE)
            .build();
        application
            .register(None::<&gtk::gio::Cancellable>)
            .expect("register test application");
        let parent = adw::ApplicationWindow::new(&application);

        prompt.render(Some(&notice), &parent);
        let (window, body, retry, dismiss) = prompt.test_widgets();

        assert!(window.is_visible());
        assert_eq!(body.text(), notice.body);
        assert_eq!(
            retry.label().as_deref(),
            Some(crate::l10n::unfiled_copy_retry())
        );
        assert_eq!(
            dismiss.label().as_deref(),
            Some(crate::l10n::unfiled_copy_dismiss())
        );
        retry.emit_clicked();
        assert!(matches!(
            receiver.recv_sync(),
            Some(AppInput::RetryUnfiledCopy)
        ));
        dismiss.emit_clicked();
        assert!(matches!(
            receiver.recv_sync(),
            Some(AppInput::DismissUnfiledCopy)
        ));
        assert!(window.is_visible(), "the core owns the close edge");

        prompt.render(
            Some(&UnfiledCopyNotice {
                retrying: true,
                ..notice
            }),
            &parent,
        );
        assert!(!retry.is_sensitive());
        assert!(!dismiss.is_sensitive());
        prompt.render(None, &parent);
        assert!(!window.is_visible());
    }
}
