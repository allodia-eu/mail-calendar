//! Why a server is reached only over plaintext, in the app's words for the engine's.

/// Why a server is reached only over plaintext, so that no sign-in is sent to it: the engine's
/// `CredentialFailureKind`, read once where a first connect is judged (`error.rs`), and carried
/// to every surface that says it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InsecureCause {
    /// The address the account has for the server is a plaintext one, `http://` off this device
    /// (the engine's `Insecure`). A secure address for the server clears it, where the server
    /// has one.
    PlaintextAddress,
    /// The server led the sign-in to plaintext from an address the engine accepts: a redirect, or
    /// a discovery step naming a plaintext address (the engine's `LedToInsecure`). Usually only
    /// the server's operator clears it, by serving that address securely; pointing the account
    /// straight at a secure address for the same service clears it too, where one exists.
    LedToPlaintext,
}
