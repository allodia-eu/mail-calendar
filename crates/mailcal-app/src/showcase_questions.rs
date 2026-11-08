//! The three questions the core can ask, raised on demand for the showcase's UI suites and help
//! screenshots (`docs/notices.md` rule 8): none of them can be produced by the showcase's bundled
//! accounts, which neither send nor answer invitations.

use engine_api::{AccountId, Draft, EmailAddress, MessageIdHeader, Provider};

use crate::{App, InvitationResponse, MessageRef, invitations_imip::ReplySource};

impl<P: Provider> App<P> {
    /// Raises a sent message whose copy is not in Sent and an invitation reply the server did
    /// not pass on, both on `account`, whose address is `address`. Answering either does
    /// nothing a person would notice: the message is not in the store to file or re-derive.
    #[doc(hidden)]
    pub fn stage_showcase_questions(&self, account: &AccountId, address: &str) {
        if let Ok(message_id) = MessageIdHeader::new("showcase-unfiled@example.org") {
            let draft = Draft::new(
                message_id,
                EmailAddress::new(address),
                vec![EmailAddress::new("team@example.org")],
                "Quarterly figures",
                "",
            );
            self.note_unfiled_copy(account, &draft, "showcase");
        }
        let Some(invitation) =
            MessageRef::from_parts(account.as_str(), "showcase-invitation".to_owned())
        else {
            return;
        };
        self.set_reply_prompt(Some(crate::ReplyPrompt {
            account: account.clone(),
            source: ReplySource::Message(invitation),
            summary: "Planning review".to_owned(),
            organizer: "organiser@example.org".to_owned(),
            response: InvitationResponse::Accept,
            comment: None,
            status_code: "5.2".to_owned(),
        }));
    }
}
