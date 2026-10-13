//! Opening a composer someone else filled in: a mail link (`docs/os-integration.md`) and an
//! assistant's draft (`docs/mcp.md`). A share, which also brings files, is
//! [`super::composer_share`].

use mailcal_bindings::{AgentDraft, ComposeOrigin, ComposePrefill, MailtoPrefill};

use super::{AppInput, AppModel, PrimaryView, composer_open::ComposerTarget};

impl AppModel {
    /// Opens a mail link's composer: a new message from the folder's account, pre-filled with
    /// what the link named (`docs/os-integration.md`). One arriving before the first account is
    /// kept until there is one.
    pub(super) fn open_mailto(&mut self, prefill: MailtoPrefill, sender: &relm4::Sender<AppInput>) {
        if self.snapshot.accounts.is_empty() {
            self.pending_mailto = Some(prefill);
            self.primary = PrimaryView::Mail;
            return;
        }
        self.open_composer_on(
            ComposeOrigin::New {
                account: self.snapshot.selected_account.clone(),
            },
            Some(mailto_prefill(prefill)),
            ComposerTarget::Pane,
            Vec::new(),
            sender,
        );
    }

    /// Opens an assistant's draft: a reply when it names the message it answers, so it keeps its
    /// thread, and a new message otherwise (`docs/mcp.md`).
    pub(super) fn open_agent_draft(&mut self, draft: AgentDraft, sender: &relm4::Sender<AppInput>) {
        // The composer lives in the mail surface, and a draft can arrive while another is up.
        self.primary = PrimaryView::Mail;
        let origin = agent_origin(&draft, self.snapshot.selected_account.as_deref());
        self.open_composer_on(
            origin,
            Some(agent_prefill(draft)),
            ComposerTarget::Pane,
            Vec::new(),
            sender,
        );
    }

    pub(super) fn try_open_pending_mailto(&mut self, sender: &relm4::Sender<AppInput>) {
        if !self.snapshot.accounts.is_empty()
            && let Some(prefill) = self.pending_mailto.take()
        {
            self.open_mailto(prefill, sender);
        }
    }
}

/// A mail link's fields, laid over a new message. Its body is text, never markup, and it brings
/// no sign-off of its own.
fn mailto_prefill(prefill: MailtoPrefill) -> ComposePrefill {
    ComposePrefill {
        to: prefill.to,
        cc: prefill.cc,
        bcc: prefill.bcc,
        subject: prefill.subject,
        body_text: prefill.body,
        signed: false,
    }
}

/// What an assistant's draft opens on: a reply to the message it names, else a new message from
/// the account it names, else from the folder's.
fn agent_origin(draft: &AgentDraft, selected_account: Option<&str>) -> ComposeOrigin {
    match (&draft.reply_to_account, &draft.reply_to_key) {
        (Some(account), Some(key)) => ComposeOrigin::Reply {
            account: account.clone(),
            key: key.clone(),
            all: false,
        },
        _ => ComposeOrigin::New {
            account: draft
                .account
                .clone()
                .or_else(|| selected_account.map(str::to_owned)),
        },
    }
}

/// An assistant's fields, laid over what it opens on. Its body carries its own sign-off, so the
/// account's signature is not seeded under it.
fn agent_prefill(draft: AgentDraft) -> ComposePrefill {
    ComposePrefill {
        to: draft.to,
        cc: draft.cc,
        bcc: draft.bcc,
        subject: draft.subject,
        body_text: draft.body_text,
        signed: true,
    }
}

#[cfg(test)]
mod tests {
    use mailcal_bindings::{AgentDraft, ComposeOrigin, ComposePrefill, MailtoPrefill};

    use super::{agent_origin, agent_prefill, mailto_prefill};

    fn draft(reply_to_account: Option<&str>, reply_to_key: Option<&str>) -> AgentDraft {
        AgentDraft {
            account: Some("work".to_owned()),
            to: "ada@example.test".to_owned(),
            cc: "copy@example.test".to_owned(),
            bcc: "audit@example.test".to_owned(),
            subject: "Planning".to_owned(),
            body_text: "A complete body and sign-off".to_owned(),
            reply_to_account: reply_to_account.map(str::to_owned),
            reply_to_key: reply_to_key.map(str::to_owned),
        }
    }

    #[test]
    fn a_mail_link_fills_a_new_message_and_brings_no_sign_off() {
        let prefill = mailto_prefill(MailtoPrefill {
            to: "ada@example.test".to_owned(),
            cc: "copy@example.test".to_owned(),
            bcc: "audit@example.test".to_owned(),
            subject: "Lunch".to_owned(),
            body: "Hello </script>".to_owned(),
        });
        assert_eq!(
            prefill,
            ComposePrefill {
                to: "ada@example.test".to_owned(),
                cc: "copy@example.test".to_owned(),
                bcc: "audit@example.test".to_owned(),
                subject: "Lunch".to_owned(),
                body_text: "Hello </script>".to_owned(),
                signed: false,
            }
        );
    }

    /// An assistant answering a message keeps the thread: opened as a new message, its reply
    /// would lose the conversation it belongs to.
    #[test]
    fn an_assistant_reply_opens_as_a_reply_to_the_message_it_names() {
        assert_eq!(
            agent_origin(&draft(Some("work"), Some("original")), Some("selected")),
            ComposeOrigin::Reply {
                account: "work".to_owned(),
                key: "original".to_owned(),
                all: false,
            }
        );
    }

    #[test]
    fn an_assistant_draft_naming_no_message_is_new_mail_from_its_account() {
        assert_eq!(
            agent_origin(&draft(Some("work"), None), Some("selected")),
            ComposeOrigin::New {
                account: Some("work".to_owned())
            },
            "half a reference is not a reply"
        );
        let unnamed = AgentDraft {
            account: None,
            ..draft(None, None)
        };
        assert_eq!(
            agent_origin(&unnamed, Some("selected")),
            ComposeOrigin::New {
                account: Some("selected".to_owned())
            }
        );
    }

    /// The assistant wrote its own sign-off, so a seeded signature would sign the message twice.
    #[test]
    fn an_assistant_draft_brings_its_own_sign_off() {
        let prefill = agent_prefill(draft(None, None));
        assert!(prefill.signed);
        assert_eq!(prefill.bcc, "audit@example.test");
        assert_eq!(prefill.body_text, "A complete body and sign-off");
    }
}
