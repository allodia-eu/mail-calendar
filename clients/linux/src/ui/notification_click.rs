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
//! A snap is the exception. The portal knows it by its desktop-file id (`<snap>_<app>`), which is
//! no bus name, so the shell cannot call the application and signals every action instead. The
//! snap listens for that signal ([`listen_in_snap`]), and a click after it has quit starts nothing.
//!
//! What the click then does to the mailbox is [`super::notification_open`]; posting is
//! [`super::notifications`].

use adw::prelude::*;
use ashpd::{desktop::notification::NotificationProxy, zvariant::OwnedValue};
use futures::StreamExt;
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

/// Delivers the clicks the portal signals, while this process runs, when it runs as a snap.
pub(crate) fn listen_in_snap(deliver: fn(AppInput)) {
    if std::env::var_os("SNAP_INSTANCE_NAME").is_none() {
        return;
    }
    // The portal connection is process-global and lives on the shared runtime
    // ([`crate::host_runtime`]).
    let Some(runtime) = crate::host_runtime::shared() else {
        return;
    };
    runtime.spawn(async move {
        if listen(deliver).await.is_err() {
            log::warn!("could not listen for notification clicks");
        }
    });
}

async fn listen(deliver: fn(AppInput)) -> ashpd::Result<()> {
    let proxy = NotificationProxy::new().await?;
    let mut clicks = proxy.receive_action_invoked().await?;
    while let Some(click) = clicks.next().await {
        if let Some(input) = input_for_signal(click.name(), click.parameter()) {
            deliver(input);
        }
    }
    Ok(())
}

/// What a signalled click means, or `None` for an action this app never posts.
fn input_for_signal(action: &str, parameter: &[OwnedValue]) -> Option<AppInput> {
    match action.strip_prefix("app.")? {
        OPEN_MESSAGE => {
            Some(input_for_pair(parameter.first().and_then(|target| {
                Vec::<String>::try_from(target.try_clone().ok()?).ok()
            })))
        }
        OPEN_APP => Some(AppInput::ShowFromNotification),
        _ => None,
    }
}

/// What a message notification's click means.
fn input_for_message(parameter: Option<&glib::Variant>) -> AppInput {
    input_for_pair(parameter.and_then(glib::Variant::get::<Vec<String>>))
}

/// The pair [`super::notifications`] sent as the action's target, read back. A target that will
/// not decode still names this app, so the click brings the window forward rather than doing
/// nothing at all.
fn input_for_pair(pair: Option<Vec<String>>) -> AppInput {
    pair.and_then(target_of)
        .map_or(AppInput::ShowFromNotification, |target| {
            AppInput::OpenNotifiedMessage(Box::new(target))
        })
}

fn target_of(pair: Vec<String>) -> Option<NotificationTarget> {
    let [account, key] = <[String; 2]>::try_from(pair).ok()?;
    (!account.is_empty() && !key.is_empty()).then_some(NotificationTarget { account, key })
}

#[cfg(test)]
mod tests {
    use ashpd::zvariant::{OwnedValue, Value};
    use gtk::glib::{Variant, prelude::ToVariant};

    use super::{
        AppInput, input_for_message, input_for_signal, open_app_action, open_message_action,
    };
    use crate::ui::notifications::NotificationTarget;

    fn signalled(action: &str, parameter: Vec<Value<'static>>) -> Option<AppInput> {
        let parameter = parameter
            .into_iter()
            .map(|value| OwnedValue::try_from(value).expect("a value with no file descriptor"))
            .collect::<Vec<_>>();
        input_for_signal(action, &parameter)
    }

    #[test]
    fn a_signalled_click_opens_the_message_its_notification_named() {
        let carried = Value::from(vec!["work".to_owned(), "m1".to_owned()]);
        let Some(AppInput::OpenNotifiedMessage(target)) =
            signalled(&open_message_action(), vec![carried])
        else {
            panic!("a message notification's click opens its message");
        };
        assert_eq!(
            *target,
            NotificationTarget {
                account: "work".to_owned(),
                key: "m1".to_owned(),
            }
        );
    }

    #[test]
    fn a_signalled_click_that_names_no_message_only_shows_the_app() {
        assert!(matches!(
            signalled(&open_app_action(), Vec::new()),
            Some(AppInput::ShowFromNotification)
        ));
        assert!(matches!(
            signalled(&open_message_action(), vec![Value::from("work")]),
            Some(AppInput::ShowFromNotification)
        ));
    }

    #[test]
    fn a_signalled_action_this_app_never_posts_is_ignored() {
        assert!(signalled("app.quit", Vec::new()).is_none());
        assert!(signalled("open-message", Vec::new()).is_none());
    }

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
