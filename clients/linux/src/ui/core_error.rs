//! What a person is shown for a core refusal.

use mailcal_bindings::MailcalError;

use crate::l10n;

/// A refused sign-in in the person's language, since the server's reply is meant for a
/// developer and the core has logged it; anything else as the core renders it.
pub(super) fn said(error: &MailcalError) -> String {
    match error {
        MailcalError::SignInRefused { .. } => l10n::signin_refused().to_owned(),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use mailcal_bindings::MailcalError;

    use super::said;
    use crate::l10n;

    #[test]
    fn a_refused_sign_in_is_said_in_words_and_anything_else_as_the_core_put_it() {
        let refused = MailcalError::SignInRefused {
            reason: "JMAP HTTP 401: {\"status\":401}".to_owned(),
        };
        assert_eq!(said(&refused), l10n::signin_refused());
        assert_eq!(
            said(&MailcalError::Connect("no route to host".to_owned())),
            "connect: no route to host"
        );
    }
}
