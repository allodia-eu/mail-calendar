//! What the credential transaction writes to the diagnostic log, as pure functions.
//!
//! The lines live here rather than inline at the call sites for the reason
//! `mailcal_account::connect_log`'s `step_line` does: the privacy decision in a log line is
//! exactly the sort of thing that is one careless edit from leaking, and a function returning a
//! `String` can be asserted on without a logger, a process-global sink, or a network. The call
//! sites in `crate::account_registry` do nothing but emit what these return.
//!
//! # Why these lines exist at all
//!
//! Until they did, a **successful** credential write was silent. The core had just taken the write
//! over from the four hosts precisely so a refused one could be acted on, and then said nothing
//! whatsoever when it worked, so a support log showed a flawless sign-in and no evidence that the
//! credential had reached the device at all. You could infer it (a refused write rolls the add
//! back, so an added account implies a stored one), but that inference needs the reader to know the
//! transaction's ordering, which is the one thing someone reading a stranger's log does not.
//!
//! That is the same unfalsifiable shape [`crate::credential_store`] was written to remove, one
//! level up: a report that only ever appears when nothing went wrong is not a report.

use mailcal_account::account_log_handle;

/// Which half of the transaction a line is about.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CredentialOp {
    /// A credential written to the host's secure store.
    Store,
    /// A credential erased from it.
    Erase,
}

impl CredentialOp {
    /// What the line says happened, in the successful case.
    fn stored(self) -> &'static str {
        match self {
            Self::Store => {
                "stored this account's credential in the host's secure store; it will still be \
                 here at the next launch"
            }
            Self::Erase => {
                "erased this account's credential from the host's secure store; it will not come \
                 back at the next launch"
            }
        }
    }

    /// What the line says the store refused.
    fn refused(self) -> &'static str {
        match self {
            Self::Store => "REFUSED this account's credential",
            Self::Erase => "REFUSED to erase this account's credential",
        }
    }
}

/// The line for a credential write or erase that **landed**.
///
/// Named by [`account_log_handle`], never by `account_id`: an account id is an address and a
/// host, both of which [`docs/logging.md`](../../../docs/logging.md) forbids outright.
pub(crate) fn ok_line(op: CredentialOp, account_id: &str) -> String {
    format!(
        "credentials: [{}] {}",
        account_log_handle(account_id),
        op.stored(),
    )
}

/// The line for a credential write or erase the host's store **refused**, carrying the host's own
/// reason.
///
/// `error` is [`crate::CredentialStoreError`]'s message, which is a store's status text ("the
/// Windows Credential Manager refused to store this account", an `OSStatus`); never a credential
/// and never an address, because the port hands the core a reason, not the payload it rejected.
pub(crate) fn refused_line(op: CredentialOp, account_id: &str, error: &str) -> String {
    format!(
        "credentials: [{}] the host's secure store {} ({error}){}",
        account_log_handle(account_id),
        op.refused(),
        match op {
            // Says what is *not* stored, because the alternative reading: a half-written
            // credential, is the one that would change what a reader does next.
            CredentialOp::Store => ", nothing is stored for it",
            CredentialOp::Erase => "",
        },
    )
}

#[cfg(test)]
mod tests {
    use super::{
        CredentialOp::{Erase, Store},
        ok_line, refused_line,
    };

    /// A realistic account id: an address at a host, which is what an id actually is, and both of
    /// the things the log may not carry.
    const ACCOUNT_ID: &str = "zelphina@carbuncle.test@imap.carbuncle.test";

    /// The privacy rule as a check rather than an intention, on **every** line this module can
    /// produce. `docs/logging.md` forbids an address, a username and a host in a file the app
    /// invites the user to attach to a support request, and interpolating the id instead of the
    /// handle is a one-word edit at each call site; exactly the kind of rule that needs a machine
    /// watching it, which is why `mailcal_app::tests_contacts_logging` exists for contacts.
    #[test]
    fn no_line_carries_the_address_the_host_or_the_id() {
        let lines = [
            ok_line(Store, ACCOUNT_ID),
            ok_line(Erase, ACCOUNT_ID),
            refused_line(Store, ACCOUNT_ID, "the keychain is locked"),
            refused_line(Erase, ACCOUNT_ID, "the keychain is locked"),
        ];
        for line in &lines {
            for forbidden in ["zelphina", "carbuncle", "@", ACCOUNT_ID] {
                assert!(
                    !line.contains(forbidden),
                    "{forbidden:?} reached a credential log line: {line}",
                );
            }
        }
    }

    /// Every line names *which* account, so one account's transaction can be followed through a
    /// log in which several are live. A line that says a credential was stored without saying
    /// whose is unreadable on any real device; they all hold more than one account.
    #[test]
    fn every_line_names_the_account_by_its_handle() {
        let handle = mailcal_account::account_log_handle(ACCOUNT_ID);
        for line in [
            ok_line(Store, ACCOUNT_ID),
            ok_line(Erase, ACCOUNT_ID),
            refused_line(Store, ACCOUNT_ID, "locked"),
            refused_line(Erase, ACCOUNT_ID, "locked"),
        ] {
            assert!(line.contains(&handle), "no account handle in: {line}");
        }
    }

    /// The two halves must not read alike. A store and an erase have opposite consequences at the
    /// next launch, and a support reader distinguishes them by this text alone.
    #[test]
    fn a_store_and_an_erase_say_opposite_things_about_the_next_launch() {
        assert!(ok_line(Store, ACCOUNT_ID).contains("still be here at the next launch"));
        assert!(ok_line(Erase, ACCOUNT_ID).contains("not come back at the next launch"));
    }

    /// A refusal carries the host's reason (the whole point of the port returning one) and a
    /// refused *write* says that nothing was stored, so it cannot be read as a partial success.
    #[test]
    fn a_refusal_carries_the_hosts_reason() {
        let line = refused_line(Store, ACCOUNT_ID, "the keychain is locked");
        assert!(line.contains("the keychain is locked"), "{line}");
        assert!(line.contains("nothing is stored for it"), "{line}");
        assert!(refused_line(Erase, ACCOUNT_ID, "locked").contains("REFUSED to erase"));
    }
}
