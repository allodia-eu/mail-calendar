//! The Allodia account half of the FFI intent surface, nested behind one `Intent::Allodia`
//! variant as the other families are. Each is handled by the binding, which holds the grant, and
//! returns at once: the card says what runs (`AllodiaCard`) until it ends.

/// A change to the Allodia account's sign-in, from its card in Settings, the first-run card or
/// a notice.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum AllodiaIntent {
    /// Sign in in the browser, which returns to `redirect_uri`, where the client listens; with
    /// `create`, at the service's sign-up page. It replaces a browser sign-in still out. What
    /// the browser does is handed back as `AccountPageIntent::SignInBrowserEnded`, as an
    /// account's sign-in is.
    SignIn { create: bool, redirect_uri: String },
    /// Stop the sign-in `attempt`, which then says nothing.
    CancelSignIn { attempt: u64 },
    /// Sign out: the stored grant is erased, and the card names the service's own page to end
    /// its session (`AllodiaCard::end_session`).
    SignOut,
}
