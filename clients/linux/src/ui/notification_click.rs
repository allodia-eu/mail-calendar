//! How a click on a new-mail notification reaches this app, running or not.
//!
//! The portal activates an action named `app.<name>` as the application's own action, through
//! `org.freedesktop.Application.ActivateAction` on the application id's bus name, and any other
//! action only by its `ActionInvoked` signal (`org.freedesktop.portal.Notification`). A signal
//! reaches a process that is subscribed to it, so it can only answer a click while this app is
//! running; a method call on the bus name is D-Bus activation, which starts the app from its
//! service file first when it is not. So the notification carries application actions, and these
//! are they.
//!
//! What the click then does to the mailbox is [`super::notification_open`]; posting is
//! [`super::notifications`].

use adw::prelude::*;
use gtk::{gio, glib};

use super::{AppInput, notifications::NotificationTarget};

/// The action a message notification's click activates, named as the application exports it.
const OPEN_MESSAGE: &str = "open-message";
/// The action the overflow summary's click activates: it names no message, so it only shows the
/// app.
const OPEN_APP: &str = "open-app";

/// The action a message notification carries, as the portal is given it.
pub(super) fn open_message_action() -> String {
    format!("app.{OPEN_MESSAGE}")
}

/// The action the overflow summary carries, as the portal is given it.
pub(super) fn open_app_action() -> String {
    format!("app.{OPEN_APP}")
}

/// Exports the two actions on `application`, handing what a click means to `deliver`.
///
/// Installed before the application runs, because a click that starts the app arrives as the
/// first thing it is asked to do: GApplication runs it straight after startup, with no
/// `activate` before it. That is also why each handler activates the application itself, which
/// builds the window the first time and only brings it forward after that.
pub(crate) fn install(application: &adw::Application, deliver: fn(AppInput)) {
    let open_message = gio::SimpleAction::new(OPEN_MESSAGE, Some(glib::VariantTy::STRING_ARRAY));
    let app = application.downgrade();
    open_message.connect_activate(move |_, parameter| {
        deliver(input_for_message(parameter));
        if let Some(app) = app.upgrade() {
            app.activate();
        }
    });
    let open_app = gio::SimpleAction::new(OPEN_APP, None);
    let app = application.downgrade();
    open_app.connect_activate(move |_, _| {
        deliver(AppInput::ShowFromNotification);
        if let Some(app) = app.upgrade() {
            app.activate();
        }
    });
    application.add_action(&open_message);
    application.add_action(&open_app);
}

/// What a message notification's click means. A target that will not decode still names this
/// app, so the click brings the window forward rather than doing nothing at all.
fn input_for_message(parameter: Option<&glib::Variant>) -> AppInput {
    parameter
        .and_then(target_of)
        .map_or(AppInput::ShowFromNotification, |target| {
            AppInput::OpenNotifiedMessage(Box::new(target))
        })
}

/// The pair [`super::notifications`] sent as the action's target, read back.
fn target_of(parameter: &glib::Variant) -> Option<NotificationTarget> {
    let [account, key] = <[String; 2]>::try_from(parameter.get::<Vec<String>>()?).ok()?;
    (!account.is_empty() && !key.is_empty()).then_some(NotificationTarget { account, key })
}

#[cfg(test)]
mod tests {
    use gtk::glib::{Variant, prelude::ToVariant};

    use super::{AppInput, input_for_message, open_app_action, open_message_action};
    use crate::ui::notifications::NotificationTarget;

    fn target(parameter: &Variant) -> Option<NotificationTarget> {
        match input_for_message(Some(parameter)) {
            AppInput::OpenNotifiedMessage(target) => Some(*target),
            _ => None,
        }
    }

    #[test]
    fn both_actions_are_the_applications_own() {
        // Only an `app.` action is delivered by activating the application. Any other name is a
        // signal, which nothing answers once the app has quit.
        assert!(open_message_action().starts_with("app."));
        assert!(open_app_action().starts_with("app."));
    }

    #[test]
    fn the_pair_a_notification_carries_is_the_message_a_click_opens() {
        let carried = vec!["work".to_owned(), "m1".to_owned()].to_variant();
        assert_eq!(
            target(&carried),
            Some(NotificationTarget {
                account: "work".to_owned(),
                key: "m1".to_owned(),
            })
        );
    }

    #[test]
    fn a_target_that_names_no_message_only_shows_the_app() {
        for carried in [
            vec!["work".to_owned()].to_variant(),
            vec!["work".to_owned(), String::new()].to_variant(),
            vec!["a", "b", "c"].to_variant(),
            "work".to_variant(),
        ] {
            assert!(target(&carried).is_none(), "{carried}");
        }
        assert!(matches!(
            input_for_message(None),
            AppInput::ShowFromNotification
        ));
    }
}
