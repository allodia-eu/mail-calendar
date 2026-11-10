//! How an account signs in, as its page in Settings draws it (`docs/accounts.md` rule 17): the
//! line under the section's heading, the ways in it offers through a browser, and the field a
//! password is typed into. A client words each and keeps no rule about which is offered.

use crate::ControlState;

#[path = "records_browser_sign_in.rs"]
mod browser;
pub use browser::{BrowserSignIn, BrowserSignInStage, SignInBrowserEnd};

/// An account's Sign-in section.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct AccountSignIn {
    /// What the section says under its heading.
    pub line: AccountSignInLine,
    /// The ways in through a browser, in the order drawn. Each one but signing a Microsoft or
    /// Google account in again waits on its server saying it takes it, which the core asks when
    /// the page opens.
    pub routes: Vec<AccountSignInRoute>,
    /// The routes' control: held while a change runs on the page, and running while the account's
    /// own sign-in starts or completes. A route pressed while the browser waits starts it again.
    pub control: ControlState,
    /// A password typed here, for an account that takes one in this section.
    pub password: Option<AccountPasswordField>,
    /// Another way in is offered, so the page says beneath the section that the account keeps
    /// its mail, settings and links whichever it uses.
    pub switches: bool,
    /// The account's browser sign-in, while one is out: the section says it waits on the browser
    /// and offers its Cancel, whichever control started it.
    #[uniffi(default = None)]
    pub browser: Option<BrowserSignIn>,
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

/// A way in through a browser, sent as `AccountPageIntent::SignIn`.
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

/// A mailbox's calendar and contacts on a server its sign-in does not reach, or that refused it
/// (`docs/accounts.md` rule 18): the form that gives them a sign-in of their own, as an account
/// linked to the mailbox. Drawn on the account's page and as setup's step after the add.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct AccountSeparateServer {
    /// The uses that move to the account the sign-in makes, calendar before contacts.
    pub uses: Vec<crate::AccountCapability>,
    /// The server's host, with its port when it names one: what the form says the sign-in is
    /// for. Never a path, which can carry the person's address.
    pub host: String,
    /// The login the form is drawn with: the one being tried while Connect runs, else the one the
    /// last connect was refused with, else the login the mailbox holds there, else its address.
    pub username: String,
    /// The way in the server offers above the password, once it has said so; the core asks it
    /// when the page opens. `None` while it takes only a password.
    pub sign_in: Option<AccountSeparateSignIn>,
    /// Connect, sent as `AccountPageIntent::ConnectSeparateServer` with what was typed, running
    /// while the password is tried.
    pub connect: ControlState,
    /// Setup's Skip, sent as `AccountPageIntent::StopUsingSeparateServer`: the account stops
    /// being used for `uses`, running while they are switched off.
    pub stop_using: ControlState,
    /// Why the last Connect, Skip or browser sign-in left the uses where they are, until another
    /// is pressed. One that worked leaves no form to draw.
    pub failed: Option<crate::AccountChangeRefusal>,
    /// The `sign_in` way in, sent as `AccountPageIntent::SignInSeparateServer`: running while the
    /// core starts the sign-in, while the browser comes up and while it completes, and pressable
    /// again while the browser waits, which starts a fresh one.
    pub sign_in_control: ControlState,
    /// The browser sign-in out for these uses, drawn as the line saying it waits on the browser
    /// with its Cancel. Connect is held while it is out.
    #[uniffi(default = None)]
    pub browser: Option<crate::BrowserSignIn>,
}

/// The way in a separate server offers besides its password, signed in through as
/// `AccountPageIntent::SignInSeparateServer`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum AccountSeparateSignIn {
    /// At the server's own provider, in the browser (`docs/mail-oauth.md` rule 13).
    Provider,
    /// With Nextcloud, in the browser (`docs/nextcloud-sign-in.md`).
    Nextcloud,
}
