//! Changing a preference: one intent, applied by the core before `dispatch` returns and saved
//! behind it, so the next read is the new value and nothing here waits on the disk. A save that
//! fails is the core's notice, drawn with the others.

use mailcal_bindings::{Intent, MailcalApp, PreferenceIntent};

pub(crate) fn set(app: &MailcalApp, intent: PreferenceIntent) {
    app.dispatch(Intent::Preferences { intent });
}
