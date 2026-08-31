//! FFI error type shared by every binding module.

/// An error building or driving a real account-backed [`crate::MailcalApp`]. Carries a
/// message rather than the source type so it crosses the FFI as a plain error a host can
/// surface.
#[derive(Debug, uniffi::Error, thiserror::Error)]
pub enum MailcalError {
    /// The account config could not be loaded or parsed.
    #[error("config: {0}")]
    Config(String),
    /// Connecting or logging in to the provider failed.
    #[error("connect: {0}")]
    Connect(String),
    /// The engine could not be opened (or the account id was invalid).
    #[error("engine: {0}")]
    Engine(String),
    /// A composer document could not be parsed, validated, or rendered.
    #[error("composer: {0}")]
    Composer(String),
}
