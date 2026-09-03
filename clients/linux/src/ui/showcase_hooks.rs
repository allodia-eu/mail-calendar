//! The debug-only hooks that drive a client to a named screen: the screenshot run
//! (`MAILCAL_SHOWCASE`) and the `MAILCAL_OPEN_SUBJECT` / `MAILCAL_OPEN_CONTACT` open hooks.
//!
//! Compiled out of a release build with the rest of that path, and kept beside the model rather
//! than inside it so the shell's own file stays the shell's.

use super::{AppInput, AppModel, ComposeKind, OpenedMessage};
use crate::showcase::ShowcaseScreen;

impl AppModel {
    /// Drives a screenshot run to the screen it asked for.
    ///
    /// The screens that need only local state are set here; the ones that need a window go through
    /// the input sender, because `init` runs before the widgets exist. `Reply` is the one that
    /// cannot finish here at all: the composer seeds itself from the opened message's *body*, which
    /// arrives on the observer, so it opens the message now and finishes in
    /// [`Self::continue_showcase`]. Waiting on the event rather than on a timer is what keeps the
    /// capture honest on a slow, cold launch.
    pub(super) fn begin_showcase(&mut self, input: &relm4::Sender<AppInput>) {
        use crate::showcase;

        if !showcase::is_on() {
            return;
        }
        // `main` has already refused an unreachable name and exited, so anything else is a bug
        // here rather than a bad flag.
        let Ok(screen) = showcase::screen() else {
            return;
        };
        match screen {
            ShowcaseScreen::List => self.open_row(0),
            ShowcaseScreen::Reply => {
                let target = showcase::reply_target();
                self.open_showcase_message(&target.account, &target.message_key, "reply");
                self.showcase_pending = Some(ShowcaseScreen::Reply);
            }
            // The card renders itself once the reading snapshot lands, so opening the message is
            // the whole of it: unlike `Reply`, nothing here waits on the message *body*. The two
            // arrangements the screen depends on are the core's: the seed files the meeting on the
            // calendar under the same `UID`, and the showcase boot refreshes the calendar before
            // any message is opened, so the card reads its answer off the diary rather than
            // honestly reporting "we have not looked at your calendar yet".
            ShowcaseScreen::Invitation => {
                let target = showcase::invitation_target();
                self.open_showcase_message(&target.account, &target.message_key, "invitation");
            }
            // Both open the same window; which category it lands on is decided where the window
            // is built, from the same flag.
            ShowcaseScreen::Settings | ShowcaseScreen::Signatures => {
                input.emit(AppInput::OpenSettings);
            }
            ShowcaseScreen::AddAccount => input.emit(AppInput::OpenAccountSetup),
            ShowcaseScreen::Calendar => self.show_calendar(),
        }
    }

    /// Opens the seeded message a screenshot is about, or exits.
    ///
    /// Refusing rather than carrying on, for the reason `showcase::parse_screen` refuses an
    /// unreachable name: a run that silently skipped its screen photographs the mailbox list, files
    /// it under that screen's name, and nothing downstream can tell.
    fn open_showcase_message(&mut self, account: &str, key: &str, screen: &str) {
        let found = self.snapshot.rows.iter().position(|row| {
            let message = OpenedMessage::from_row(row);
            message.account == account && message.key == key
        });
        let Some(index) = found else {
            eprintln!(
                "the showcase {screen} target is not in the seeded mailbox: refusing to \
                 photograph the message list under the {screen} screen's name"
            );
            std::process::exit(2);
        };
        self.open_row(index);
    }

    /// Finishes a `reply` screenshot once the opened message's body has arrived.
    pub(super) fn continue_showcase(&mut self) {
        // `body_arrived`, not `matches_opened`: the core publishes the reading surface as soon as
        // the selection changes, and `quote_seed` refuses a snapshot with no body: so replying on
        // the first notification opens an empty composer with neither the quote nor the sample
        // text, which is a well-formed screenshot of the wrong thing.
        if self.showcase_pending != Some(ShowcaseScreen::Reply) || !self.reading.body_arrived() {
            return;
        }
        self.showcase_pending = None;
        self.begin_compose(ComposeKind::Reply);
    }

    pub(super) fn apply_debug_open_hook(&mut self) {
        if self.reading.opened.is_some() {
            return;
        }
        let Some(requested) = std::env::var("MAILCAL_OPEN_SUBJECT")
            .ok()
            .filter(|subject| !subject.is_empty())
        else {
            return;
        };
        if let Some(index) = debug_open_subject_index(&self.snapshot.rows, &requested) {
            self.open_row(index);
        }
    }

    /// Opens the person named by `MAILCAL_OPEN_CONTACT`, once.
    ///
    /// A GTK list row exposes no AT-SPI action, so the driver cannot open a person, and
    /// everything the detail leads to (the "Also in" explanation, the edit affordance, the
    /// editor itself) would be unreachable from a test. This is the same escape hatch
    /// `MAILCAL_OPEN_SUBJECT` is for the reading pane, and for the same reason.
    ///
    /// Matched on the **displayed** name, which is what a test can read off the screen: a
    /// nameless card carries the client's placeholder there, and so does this.
    pub(super) fn apply_debug_open_contact_hook(&mut self, sender: relm4::Sender<AppInput>) {
        if self.contacts.opened().is_some() {
            return;
        }
        let Some(requested) = std::env::var("MAILCAL_OPEN_CONTACT")
            .ok()
            .filter(|name| !name.is_empty())
        else {
            return;
        };
        if let Some(id) = debug_open_contact_id(self.contacts.rows(), &requested) {
            self.open_contact(id, sender);
        }
    }
}

fn debug_open_contact_id(
    rows: &[crate::ui::contacts::PersonRow],
    requested: &str,
) -> Option<String> {
    rows.iter()
        .find(|row| row.name == requested)
        .map(|row| row.id.clone())
}

fn debug_open_subject_index(
    rows: &[mailcal_bindings::SnapshotRow],
    requested: &str,
) -> Option<usize> {
    rows.iter()
        .position(|row| OpenedMessage::from_row(row).subject == requested)
}

#[cfg(test)]
mod tests {
    use mailcal_bindings::{FlatRow, SnapshotRow};

    use super::{debug_open_contact_id, debug_open_subject_index};

    fn row(subject: &str) -> SnapshotRow {
        SnapshotRow::Flat {
            row: FlatRow {
                avatar: crate::ui::model::blank_avatar(),
                account: "fixture".to_owned(),
                key: subject.to_owned(),
                subject: subject.to_owned(),
                from: "sender@example.test".to_owned(),
                date: "2026-07-20".to_owned(),
                unread: true,
                flagged: false,
                has_attachment: false,
                preview: String::new(),
            },
        }
    }

    #[test]
    fn the_debug_open_hook_selects_only_an_exact_subject() {
        let rows = vec![row("First"), row("HTML message with a remote image")];

        assert_eq!(
            debug_open_subject_index(&rows, "HTML message with a remote image"),
            Some(1)
        );
        assert_eq!(debug_open_subject_index(&rows, "HTML message"), None);
    }

    #[test]
    fn the_debug_contact_hook_selects_only_an_exact_name() {
        let person = |id: &str, name: &str| crate::ui::contacts::PersonRow {
            id: id.to_owned(),
            name: name.to_owned(),
            email: format!("{id}@example.test"),
            avatar: (&crate::ui::model::blank_avatar()).into(),
            section: None,
            accounts: None,
        };
        let rows = [person("1", "Ada Lovelace"), person("2", "Grace Hopper")];

        assert_eq!(
            debug_open_contact_id(&rows, "Grace Hopper").as_deref(),
            Some("2")
        );
        assert_eq!(debug_open_contact_id(&rows, "Grace"), None);
    }
}
