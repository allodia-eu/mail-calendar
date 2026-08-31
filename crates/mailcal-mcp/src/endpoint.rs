//! Where the server listens, and whether that place is usable.
//!
//! The **host passes the endpoint down**, exactly as it already passes `data_dir`: so a
//! sandboxed build can differ from a Developer-ID one without a `#[cfg]` anywhere in Rust, and
//! the Settings screen can put the very same string into the config snippet it tells the user to
//! paste. One value, derived once, by the layer that knows the answer.

/// The `sun_path` limit for a Unix domain socket address, in bytes, **including** the trailing
/// NUL. 104 on macOS and the BSDs, 108 on Linux; the smaller is used everywhere so a path that
/// validates on one platform validates on all of them.
pub const MAX_UNIX_PATH: usize = 104;

/// Why an endpoint cannot be listened on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EndpointError {
    /// The endpoint string was empty.
    Empty,
    /// A Unix socket path that does not fit in `sun_path`.
    ///
    /// This one is worth a variant of its own. The macOS sandbox rewrites `~/.local/share/…`
    /// into `~/Library/Containers/eu.allodia.mailcal/Data/…`, which is **93 bytes before the
    /// username and the file name are counted**: so a long username plus a dev subdirectory
    /// overflows the limit and `bind()` fails with `ENAMETOOLONG`, an error nobody reading
    /// "the MCP server did not start" will connect to a path length. The check exists so that
    /// failure is a sentence instead of a mystery, and there is a test pinning it.
    TooLong {
        /// How many bytes the path needed.
        length: usize,
    },
    /// A Windows pipe name that is not under `\\.\pipe\`.
    NotAPipeName,
}

impl core::fmt::Display for EndpointError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Empty => write!(f, "the MCP endpoint is empty"),
            Self::TooLong { length } => write!(
                f,
                "the MCP socket path is {length} bytes; a Unix socket allows at most \
                 {} including the terminator",
                MAX_UNIX_PATH - 1,
            ),
            Self::NotAPipeName => write!(f, "a Windows MCP endpoint must be a \\\\.\\pipe\\ name"),
        }
    }
}

/// A validated place to listen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Endpoint {
    /// A Unix domain socket at this filesystem path.
    Unix(String),
    /// A Windows named pipe, `\\.\pipe\…`.
    Pipe(String),
}

impl Endpoint {
    /// Validates a host-supplied endpoint string.
    ///
    /// A value starting `\\.\pipe\` is a named pipe; anything else is a filesystem path. The
    /// discrimination is on the value rather than on `cfg!(windows)` so the rule is testable on
    /// any host: a Windows pipe name is malformed on Linux too, and finding that out in a unit
    /// test beats finding it out on the one machine that can run it.
    ///
    /// # Errors
    ///
    /// See [`EndpointError`].
    pub fn parse(raw: &str) -> Result<Self, EndpointError> {
        let raw = raw.trim();
        if raw.is_empty() {
            return Err(EndpointError::Empty);
        }
        if raw.starts_with(r"\\.\pipe\") {
            if raw.len() <= r"\\.\pipe\".len() {
                return Err(EndpointError::NotAPipeName);
            }
            return Ok(Self::Pipe(raw.to_owned()));
        }
        // `sun_path` counts bytes, not characters, and holds a trailing NUL.
        let length = raw.len() + 1;
        if length > MAX_UNIX_PATH {
            return Err(EndpointError::TooLong { length });
        }
        Ok(Self::Unix(raw.to_owned()))
    }

    /// The endpoint as the string a host would round-trip.
    #[must_use]
    pub fn as_str(&self) -> &str {
        match self {
            Self::Unix(path) | Self::Pipe(path) => path,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Endpoint, EndpointError, MAX_UNIX_PATH};

    #[test]
    fn a_socket_path_at_the_sun_path_limit_is_rejected_before_bind_fails_mysteriously() {
        // THE finding this check encodes. `bind()` returns ENAMETOOLONG and nothing in the
        // resulting log mentions a length, so without this the symptom is "MCP just doesn't
        // start" on exactly the machines with long usernames.
        let ok = "/".repeat(MAX_UNIX_PATH - 1);
        assert!(matches!(Endpoint::parse(&ok), Ok(Endpoint::Unix(_))));

        let too_long = "/".repeat(MAX_UNIX_PATH);
        assert_eq!(
            Endpoint::parse(&too_long),
            Err(EndpointError::TooLong {
                length: MAX_UNIX_PATH + 1
            }),
        );
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn the_real_home_relative_path_this_product_uses_fits_on_macos() {
        // The sandbox container path (~/Library/Containers/eu.allodia.mailcal/Data/.local/
        // share/mailcal/mcp.sock) is 93 bytes BEFORE the username, so it overflows for anyone
        // whose home directory name is more than a few characters. The real-home path is 44.
        // This asserts the shape the product actually ships is safely inside the limit.
        let home = std::env::var("HOME").expect("a home directory");
        let path = format!("{home}/.local/share/mailcal/mcp.sock");
        assert!(
            path.len() < MAX_UNIX_PATH,
            "the shipped socket path is {} bytes, over the {MAX_UNIX_PATH}-byte sun_path limit",
            path.len() + 1,
        );
        assert!(matches!(Endpoint::parse(&path), Ok(Endpoint::Unix(_))));
    }

    #[test]
    fn a_pipe_name_is_recognized_on_every_host_so_the_rule_is_testable_anywhere() {
        assert_eq!(
            Endpoint::parse(r"\\.\pipe\eu.allodia.mailcal.mcp"),
            Ok(Endpoint::Pipe(
                r"\\.\pipe\eu.allodia.mailcal.mcp".to_owned()
            )),
        );
        assert_eq!(
            Endpoint::parse(r"\\.\pipe\"),
            Err(EndpointError::NotAPipeName),
        );
    }

    #[test]
    fn a_dev_build_and_a_store_build_do_not_collide() {
        // Both coexist on one machine constantly during development; sharing one endpoint would
        // mean whichever started first silently owns the other's clients.
        let dev = Endpoint::parse("/Users/x/.local/share/mailcal.dev/mcp.sock").unwrap();
        let prod = Endpoint::parse("/Users/x/.local/share/mailcal/mcp.sock").unwrap();
        assert_ne!(dev, prod);

        let dev_pipe = Endpoint::parse(r"\\.\pipe\eu.allodia.mailcal.dev.mcp").unwrap();
        let prod_pipe = Endpoint::parse(r"\\.\pipe\eu.allodia.mailcal.mcp").unwrap();
        assert_ne!(dev_pipe, prod_pipe);
    }

    #[test]
    fn an_empty_endpoint_is_an_error_not_a_default() {
        assert_eq!(Endpoint::parse("   "), Err(EndpointError::Empty));
    }
}
