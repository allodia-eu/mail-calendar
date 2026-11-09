//! The shape the calendar opens in on a device nobody has chosen one for: three days on a phone,
//! the whole week elsewhere (`docs/calendar.md` §8). Asserted through the constructor every native
//! client boots through, because the class the client reports is the only thing that tells a
//! phone apart, and the default has to be in place before the client can read a setting.
//! A child of [`super`], reusing its boot fakes.

use super::*;
use crate::{CalendarLayout, DeviceClass, MailcalApp, analytics::test_device};

fn layout_on(device_class: DeviceClass, name: &str) -> CalendarLayout {
    let data_dir = temp_data_dir(name);
    let app = MailcalApp::new_accounts(
        Box::new(NoopObserver),
        Box::new(NullLogger),
        LogLevel::Info,
        Vec::new(),
        data_dir.to_string_lossy().into_owned(),
        "Etc/UTC".to_owned(),
        crate::DeviceInfo {
            device_class,
            ..test_device()
        },
        Box::new(NullCredentialStore),
    )
    .expect("an app with no accounts boots");
    let layout = app.display_settings().layout;
    drop(app);
    let _ = fs::remove_dir_all(data_dir);
    layout
}

#[test]
fn a_phone_opens_the_calendar_on_three_days() {
    for (class, name) in [
        (DeviceClass::Iphone, "layout-iphone"),
        (DeviceClass::AndroidPhone, "layout-android-phone"),
    ] {
        assert!(
            matches!(layout_on(class, name), CalendarLayout::ThreeDay),
            "{name}"
        );
    }
}

#[test]
fn a_tablet_or_a_computer_opens_it_on_the_week() {
    for (class, name) in [
        (DeviceClass::Ipad, "layout-ipad"),
        (DeviceClass::AndroidTablet, "layout-android-tablet"),
        (DeviceClass::MacLaptop, "layout-mac"),
        (DeviceClass::Pc, "layout-pc"),
        (DeviceClass::LinuxDesktop, "layout-linux"),
    ] {
        assert!(
            matches!(layout_on(class, name), CalendarLayout::Week),
            "{name}"
        );
    }
}
