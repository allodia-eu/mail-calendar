//! What a Microsoft or Google account opens with the grant it holds, and what a completed sign-in
//! keeps of the account it signs in again.

use mailcal_account::{AccountShape, Capabilities, Capability};

use crate::MailcalError;

/// The uses an account opens: what it is chosen for, less what its grant withholds.
///
/// Mail is opened even when withheld. Its failure is the account's, and a grant without the mail
/// scope is answered by the sync or send that is refused, which is where the prompts to sign in
/// again are raised; holding it back here would leave a mail account silently without mail.
pub(crate) fn opened(chosen: &Capabilities, withheld: &Capabilities) -> Capabilities {
    chosen
        .iter()
        .filter(|capability| *capability == Capability::Mail || !withheld.contains(*capability))
        .collect()
}

/// Refuses a sign-in whose grant allows nothing the account was chosen for, rather than adding an
/// account that opens nothing.
///
/// # Errors
///
/// Returns [`MailcalError::Connect`] when no chosen use survives the grant.
pub(crate) fn refuse_an_empty_grant(
    chosen: &Capabilities,
    withheld: &Capabilities,
) -> Result<(), MailcalError> {
    let open = opened(chosen, withheld);
    let usable = [Capability::Mail, Capability::Calendar, Capability::Contacts]
        .into_iter()
        .any(|capability| open.contains(capability));
    if usable {
        Ok(())
    } else {
        Err(MailcalError::Connect(
            "the provider allowed none of what this account was chosen for".to_owned(),
        ))
    }
}

/// Carries what an existing account stored over to the config a new sign-in for it produced: its
/// pinned id and links always, its capabilities unless the sign-in made a choice of its own.
pub(crate) fn keep_stored_shape(signed_in: &mut AccountShape, stored: Option<AccountShape>) {
    let Some(stored) = stored else {
        return;
    };
    signed_in.id = stored.id;
    signed_in.links = stored.links;
    if signed_in.capabilities.is_none() {
        signed_in.capabilities = stored.capabilities;
    }
}

#[cfg(test)]
#[path = "consent_tests.rs"]
mod tests;
