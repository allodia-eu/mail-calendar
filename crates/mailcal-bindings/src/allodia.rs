//! Signing in to an **Allodia account**, over the FFI: the records every client binds to, and the
//! stored shape the grant is kept in.
//!
//! An Allodia account is not a mail account. It carries no mailbox, appears in no switcher, and a
//! token issued for it cannot touch anyone's mail; what it is for is asking Allodia's own service
//! what the person is entitled to. So it lives beside the account list rather than in it, and the
//! setup wizard never offers it; its screen is Settings. The rules it follows are the
//! `entitlement.md` contract that ships beside the Allodia Licence, and this file states none of
//! them: a build of the open tree does not have that directory to read, which is the whole point of
//! the seam.
//!
//! # Why the surface is here rather than under a `cfg`
//!
//! The three exported items below exist in **every** build, and only their bodies change. A build
//! that carries no Allodia registration answers `false` and refuses the flow, which is precisely
//! what a build from source is: the same absent-is-supported rule every other provider follows
//! ([`crate::oauth_routes`]). Putting the exports themselves behind the feature would generate two
//! different FFI surfaces, and the four clients in this repository could then only compile against
//! one of them.
//!
//! # Where the grant is kept
//!
//! In the host's secure store, through the same [`AccountCredentialStore`](crate::credential_store)
//! every mail account uses, under the reserved id [`ACCOUNT_ID`]. That means no new port, no new
//! host code, and a rotated refresh token has somewhere to land. It also means the entry comes back
//! at the next launch in the same `configs` list as the mail accounts: so `boot` routes it out
//! before anything tries to read it as a mailbox, and a build **without** the feature routes it out
//! too, ignoring it rather than reporting a perfectly good grant as a corrupt account.

use serde::{Deserialize, Serialize};

/// The reserved id the Allodia grant is stored under.
///
/// A host keys its secure store on whatever id the core hands it and asks no questions, so this has
/// only to be an id no mail account can derive. It cannot: every mail account's id is derived from
/// its address and protocol, and none of them is this literal.
pub(crate) const ACCOUNT_ID: &str = "allodia-account";

/// Who is signed in to an Allodia account.
///
/// The address is what identifies the account. Nothing here says what the account may *do*, that
/// is the entitlement's answer, and the server resolves it from the token rather than from anything
/// a client holds.
#[derive(uniffi::Record, Debug, Clone, PartialEq, Eq)]
pub struct AllodiaAccount {
    /// The account's email address.
    pub email: String,
    /// The person's display name, when the service holds one.
    pub name: Option<String>,
}

/// What starting a sign-in returns: the page to open, and an opaque handle to hand back.
#[derive(uniffi::Record, Debug, Clone)]
pub struct AllodiaSignInStart {
    /// The authorization URL to open in the platform auth session / default browser.
    pub authorization_url: String,
    /// An opaque handle (the CSRF `state`, the PKCE verifier and what discovery found) to pass to
    /// `complete_allodia_sign_in`. Transient; hold it in memory only, never on disk: it carries
    /// the verifier that protects the code exchange.
    pub pending: String,
}

/// Whether this build can offer Allodia sign-in at all.
///
/// A client asks before it draws the button, and draws nothing when the answer is `false`, which
/// is the ordinary answer for a build from source, not a failure. Cheap and constant; a client may
/// call it per screen.
#[must_use]
#[uniffi::export]
pub fn allodia_sign_in_available() -> bool {
    #[cfg(feature = "allodia-license")]
    {
        allodia_license::available()
    }
    #[cfg(not(feature = "allodia-license"))]
    {
        false
    }
}

/// The stored grant, as it sits in the host's secure store.
///
/// TOML in an `[allodia]` table, because that is what the store already holds and what makes this
/// entry recognisable the way every other kind is: the four mail kinds each name a distinct
/// top-level section, and this is a fifth that no mail parse accepts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct StoredAccount {
    /// The account's email address.
    pub(crate) email: String,
    /// The person's display name, when the service gave one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) name: Option<String>,
    /// The refresh token. The whole reason this entry is in the secure store and not in
    /// preferences.
    pub(crate) refresh_token: String,
    /// The scopes the service actually issued this grant for, when a token response named them.
    ///
    /// Kept so "may this build do X?" is answered locally, before a request that would fail. It
    /// is deliberately an `Option` and not an empty `Vec`: **absent means not known**, which is
    /// what every grant stored by a build predating this field is, and what a response that
    /// omitted `scope` leaves it as. Read as "no scopes", not-known would report every feature
    /// missing and prompt a person whose grant is perfectly good: so a `None` prompts nothing
    /// and waits for the evidence a refused request provides.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) granted_scopes: Option<Vec<String>>,
    /// Where to end the browser session, as discovery found it at sign-in.
    ///
    /// Kept beside the grant so signing out needs no network round trip *before* it can erase
    /// anything: a sign-out that had to discover first could fail before it started. Absent for a
    /// grant stored by a build that did not record it, and for a service advertising none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) end_session_endpoint: Option<String>,
}

/// The document wrapper, so the table is named on disk.
#[derive(Debug, Serialize, Deserialize)]
struct StoredDocument {
    allodia: StoredAccount,
}

impl StoredAccount {
    /// The stored form. Fails only if the values cannot be represented, which they always can.
    ///
    /// Only a build that can sign in ever writes one: a build without the feature reads the entry
    /// so it can leave it alone, and has nothing to put there. The tests write one either way,
    /// because what they pin is the round trip.
    #[cfg(any(feature = "allodia-license", test))]
    pub(crate) fn to_toml(&self) -> Result<String, toml::ser::Error> {
        toml::to_string(&StoredDocument {
            allodia: self.clone(),
        })
    }

    /// Read a stored entry back, or `None` if this is not one.
    ///
    /// `None` is the answer for every mail account's config as well as for a corrupt entry, and the
    /// caller treats both the same way: this is not an Allodia grant.
    pub(crate) fn from_toml(text: &str) -> Option<Self> {
        toml::from_str::<StoredDocument>(text)
            .ok()
            .map(|document| document.allodia)
            .filter(|stored| !stored.email.is_empty() && !stored.refresh_token.is_empty())
    }

    /// What a client is told about it; everything except the secret.
    pub(crate) fn account(&self) -> AllodiaAccount {
        AllodiaAccount {
            email: self.email.clone(),
            name: self.name.clone(),
        }
    }
}

/// Whether a stored credential-store entry is the **Allodia account** rather than a mail account.
///
/// Two paths need it, and the second is on every launch.
///
/// **Deciding a first run.** A client shows the account-setup screen when the store holds no mail
/// account, and the store holds this entry too: so the *length* of what it hands over is not the
/// number of mail accounts. Read as though it were, signing in on the first-run screen and quitting
/// before adding a mailbox left the next launch convinced setup was finished: an empty inbox, no
/// route back to the screen that adds an account, and nothing connecting either to the sign-in.
///
/// **A debug launch that connects a canned dev account** deliberately does *not* connect the stored
/// accounts, and would otherwise drop the one entry that is not a mail account, so a sign-in made
/// in that mode looks like it never stuck.
///
/// It is asked rather than pattern-matched because the stored shape belongs here: a client that
/// looked for the section name itself would be a second reader of it, free to disagree with this
/// one the moment either moves.
#[must_use]
#[uniffi::export]
pub fn is_allodia_account_config(config: String) -> bool {
    StoredAccount::from_toml(&config).is_some()
}

/// Takes the Allodia grant out of the host's stored configs, leaving the mail accounts.
///
/// Called once at boot, before anything reads a config, and **unconditionally**: a build with no
/// Allodia sign-in still has to recognise an entry an Allodia build wrote, so it can leave it
/// alone. Without that, the entry would reach the mail parsers, fail all four, and surface at every
/// launch as "an account could not be loaded": about an account that does not exist, and a grant
/// that is perfectly intact.
///
/// A second entry would mean two sign-ins were somehow written under one reserved id. The first is
/// kept and the rest are dropped from the list, because leaving one behind would send it to the
/// mail parsers: the failure this exists to prevent.
pub(crate) fn take_stored(configs: &mut Vec<String>) -> Option<StoredAccount> {
    let mut found = None;
    configs.retain(|config| match StoredAccount::from_toml(config) {
        Some(stored) => {
            found = found.take().or(Some(stored));
            false
        }
        None => true,
    });
    if found.is_some() {
        log::info!("allodia: an account is signed in");
    }
    found
}

#[cfg(test)]
mod tests {
    use super::{StoredAccount, is_allodia_account_config, take_stored};

    /// Whether boot would route this config away from the mail parsers.
    fn routed(config: &str) -> bool {
        let mut configs = vec![config.to_owned()];
        let taken = take_stored(&mut configs);
        assert_eq!(taken.is_some(), configs.is_empty(), "{config}");
        taken.is_some()
    }

    fn stored() -> StoredAccount {
        StoredAccount {
            email: "someone@example.com".to_owned(),
            name: Some("Someone".to_owned()),
            refresh_token: "refresh-token".to_owned(),
            granted_scopes: Some(vec!["openid".to_owned(), "offline_access".to_owned()]),
            end_session_endpoint: Some("https://as.example.com/end-session".to_owned()),
        }
    }

    #[test]
    fn a_grant_stored_before_the_logout_url_existed_still_reads_back() {
        // An entry written by a previous build outlives its update, and a grant that stops
        // parsing reads to the person as having been silently signed out.
        let older = "[allodia]\nemail = \"someone@example.com\"\nrefresh_token = \"tok\"\n";
        let read = StoredAccount::from_toml(older).expect("an older entry is still a grant");
        assert_eq!(read.email, "someone@example.com");
        assert!(read.end_session_endpoint.is_none());
        // And its permissions are NOT KNOWN rather than empty, which is the difference between
        // carrying on and prompting every signed-in person on the day this ships.
        assert!(read.granted_scopes.is_none());
    }

    #[test]
    fn a_stored_grant_survives_the_round_trip() {
        let toml = stored().to_toml().expect("serializable");
        assert_eq!(StoredAccount::from_toml(&toml), Some(stored()));
    }

    #[test]
    fn a_name_the_service_did_not_give_is_absent_rather_than_empty() {
        let anonymous = StoredAccount {
            name: None,
            ..stored()
        };
        let toml = anonymous.to_toml().expect("serializable");
        assert!(!toml.contains("name"), "{toml}");
        assert_eq!(StoredAccount::from_toml(&toml), Some(anonymous));
    }

    /// The routing rule this whole entry shape exists for. Each mail kind names its own top-level
    /// section, so recognising this one is a parse and not a guess, and every mail config must
    /// answer `false`, or boot would route a real account away from its provider.
    #[test]
    fn only_the_allodia_entry_is_recognised_as_one() {
        assert!(routed(&stored().to_toml().expect("serializable")));
        for other in [
            "[imap]\naddr = \"imap.example.com:993\"\n",
            "[microsoft]\nemail = \"someone@example.com\"\n",
            "[google]\nemail = \"someone@example.com\"\n",
            "[jmap]\nbase_url = \"https://api.example.com\"\n",
            "not toml at all",
            "",
        ] {
            assert!(!routed(other), "{other}");
        }
    }

    /// The exported predicate answers the same question the router does. It has one caller; the
    /// dev-account boot that carries this entry over by hand, and the failure it prevents is a
    /// developer's sign-in disappearing at the next launch of the mode they test in.
    #[test]
    fn a_client_can_ask_which_stored_entry_is_the_allodia_one() {
        assert!(is_allodia_account_config(
            stored().to_toml().expect("serializable")
        ));
        assert!(!is_allodia_account_config(
            "[imap]\naddr = \"imap.example.com:993\"\n".to_owned()
        ));
        assert!(!is_allodia_account_config(String::new()));
    }

    /// Every client decides "is this a first run" from the stored configs, and this predicate is
    /// what makes that answerable.
    ///
    /// The store holds the Allodia grant beside the mail accounts, so a list's *length* is not the
    /// number of mail accounts. Three clients read it as though it were, and the cost was specific:
    /// sign in on the first-run screen, quit before adding a mailbox, and the next launch decided
    /// setup was finished: an empty inbox, no way back to the screen that adds an account, and
    /// nothing on screen connecting either fact to the sign-in that caused them.
    ///
    /// The condition each client now asks is exactly this: **are all of them the grant?**
    #[test]
    fn a_store_holding_only_the_grant_is_still_a_first_run() {
        let grant = stored().to_toml().expect("serializable");
        let mail = "[imap]\naddr = \"imap.example.com:993\"\n".to_owned();

        let first_run =
            |configs: Vec<String>| configs.iter().all(|c| is_allodia_account_config(c.clone()));

        assert!(first_run(vec![]), "nothing stored is a first run");
        assert!(
            first_run(vec![grant.clone()]),
            "signed in, no mailbox: still a first run, and the bug this pins"
        );
        assert!(!first_run(vec![mail.clone()]), "one mail account is not");
        assert!(
            !first_run(vec![grant, mail]),
            "nor is a mail account beside the grant"
        );
    }

    /// The other half of routing: what is taken is taken, and everything else is left in order for
    /// the mail parsers. A router that dropped a mail account would lose it at every launch.
    #[test]
    fn routing_removes_only_the_grant() {
        let mut configs = vec![
            "[imap]\naddr = \"imap.example.com:993\"\n".to_owned(),
            stored().to_toml().expect("serializable"),
            "[jmap]\nbase_url = \"https://api.example.com\"\n".to_owned(),
        ];
        assert_eq!(take_stored(&mut configs), Some(stored()));
        assert_eq!(configs.len(), 2);
        assert!(configs[0].starts_with("[imap]"));
        assert!(configs[1].starts_with("[jmap]"));
    }

    /// An entry missing the token is not a grant. It would otherwise be routed away from the mail
    /// parsers *and* be useless, which is the one outcome worse than either.
    #[test]
    fn a_half_written_entry_is_not_a_grant() {
        for text in [
            "[allodia]\nemail = \"someone@example.com\"\nrefresh_token = \"\"\n",
            "[allodia]\nemail = \"\"\nrefresh_token = \"refresh-token\"\n",
            "[allodia]\nemail = \"someone@example.com\"\n",
        ] {
            assert!(!routed(text), "{text}");
        }
    }
}
