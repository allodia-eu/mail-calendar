//! How an account signs in, as its page in Settings draws it (`docs/accounts.md` rule 17): the
//! line under the section's heading, the ways in it offers through a browser, and the field a
//! password is typed into. A client words each and keeps no rule about which is offered.

use crate::ControlState;

/// An account's Sign-in section.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct AccountSignIn {
    /// What the section says under its heading.
    pub line: AccountSignInLine,
    /// The ways in through a browser, in the order drawn. Each one but signing a Microsoft or
    /// Google account in again waits on its server saying it takes it, which the core asks when
    /// the page opens.
    pub routes: Vec<AccountSignInRoute>,
    /// The routes' control: held while a change runs on the page. A browser sign-in is still the
    /// client's to run, and it holds them while the core completes one.
    pub control: ControlState,
    /// A password typed here, for an account that takes one in this section.
    pub password: Option<AccountPasswordField>,
    /// Another way in is offered, so the page says beneath the section that the account keeps
    /// its mail, settings and links whichever it uses.
    pub switches: bool,
}

/// The line under the Sign-in heading.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum AccountSignInLine {
    /// It signs in with a password, an app password or an API token.
    Password,
    /// It signs in through its provider, in the browser.
    Provider,
    /// A Nextcloud sign-in made it, and it signs in again that way.
    Nextcloud,
    /// A Microsoft or Google account, which signs in again through its provider's own page when
    /// something about its sign-in is wrong.
    AtProvider,
    /// A Microsoft or Google account the provider has not granted every chosen use: signing in
    /// again asks for them.
    NeedsPermission,
    /// The server refused the stored sign-in, and the section holds the remedy.
    Refused,
}

/// A way in through a browser, which the client starts as it does today.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum AccountSignInRoute {
    /// Sign in again the way the account already does: through its provider, or with Nextcloud.
    SignInAgain,
    /// Switch from a password to signing in through the provider, named `label` when this build
    /// holds a registration naming it.
    ToProvider { label: Option<String> },
    /// Switch a calendar and contacts account from its typed password to signing in with
    /// Nextcloud (`docs/nextcloud-sign-in.md` rule 7).
    ToNextcloud,
}

/// The field a password is typed into, sent with `AccountPageIntent::UsePassword`. It never shows
/// the account's own credential, and is emptied as it is sent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Record)]
pub struct AccountPasswordField {
    /// What it asks for, which its label says.
    pub kind: AccountSecretKind,
    /// It replaces a password the server refused, so it is drawn at once; otherwise it switches
    /// the account from its provider's sign-in to a password, behind a button that reveals it.
    pub replaces: bool,
    /// Its Save, running while the password is tried.
    pub save: ControlState,
}

/// What a password field asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum AccountSecretKind {
    /// A password.
    Password,
    /// A password or an API token, as a JMAP server takes either.
    PasswordOrToken,
}

/// A password a person typed, on its way to the core in a page change. Never printed: a change
/// travels in a client's own inputs, and either can reach a log through its `Debug`.
#[derive(Clone, PartialEq, Eq, uniffi::Record)]
pub struct TypedPassword {
    pub text: String,
}

impl std::fmt::Debug for TypedPassword {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("TypedPassword(..)")
    }
}
