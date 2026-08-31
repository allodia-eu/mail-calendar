//! How this server introduces itself: the name a client stores it under, the name it *shows*,
//! and the icon beside it.
//!
//! MCP `2025-11-25` (this server's legacy floor) widened `serverInfo` (the spec's
//! `Implementation`) from a bare
//! `name`/`version` to also carry `title`, `description`, `websiteUrl` and `icons`. Without
//! them a client falls back to whatever key the user happened to write in their config;
//! `allodia-mail`, lowercased and hyphenated, beside a generated letter avatar. That is a
//! machine identifier standing in for a product name, in the one place a user goes to decide
//! whether to trust this thing with their mail.
//!
//! # The icon is inlined, not linked
//!
//! `src` may be an HTTPS URL or a `data:` URI. This uses a data URI, and the reason is the
//! product's own posture rather than convenience: a URL would have the client reach out to
//! `allodia.eu`; from the user's machine, on a schedule we do not control, for an app whose
//! whole claim is that it talks to nobody but their mail provider. A logo fetch is a weak
//! analytics signal, and a weak analytics signal we did not ask permission for is exactly what
//! `docs/analytics.md` exists to prevent. Inlining costs ~24 KB once per session on a local
//! socket, and it works with no network at all.
//!
//! The spec's icon guidance points the same way: *"Verify that icon URIs are from the same
//! origin as the server. This minimises the risk of exposing data or tracking information to
//! third-parties."* A local stdio server has no origin to be same as; an inlined icon sidesteps
//! the question rather than answering it badly.
//!
//! PNG, not SVG, deliberately. PNG is the one format the spec requires every icon-rendering
//! client to support, and SVG carries a scripting surface the spec warns about: a brand mark
//! is not worth handing anyone an executable payload over.

use std::sync::OnceLock;

use base64::{Engine, engine::general_purpose::STANDARD};

/// The stable machine name; **letters, numbers and hyphens only**.
///
/// Clients key sessions, logs and permission rules on this, so it does not change once shipped.
/// The character set is not cosmetic: Claude Code accepts only `[A-Za-z0-9_-]` in a server name
/// and rewrites anything else to `_` where the name is embedded in a tool identifier, so an
/// ampersand or a space here would be silently mangled somewhere. The human-readable form is
/// [`SERVER_TITLE`]'s job.
///
/// This is also the key `McpEndpoint.configurationSnippet` files the server under on the client
/// side; one identifier, not two, so a support answer matches what the user sees in both places.
pub const SERVER_NAME: &str = "allodia-mail-and-calendar";

/// The display name. **The product's full name**, per the brand rule; "Allodia Mail &
/// Calendar", never "Allodia", which is the company.
pub const SERVER_TITLE: &str = "Allodia Mail & Calendar";

/// One line of what connecting this gets you, shown by clients that surface a description.
///
/// It names the boundary rather than the feature list: someone reading this is deciding
/// whether to let a program into their mail, and "the accounts you allow" is the fact that
/// answers them.
pub const SERVER_DESCRIPTION: &str = "Read and act on the mail in the accounts you allow, from the Allodia Mail & Calendar app \
     running on this machine.";

/// Where to find out what this is.
pub const SERVER_WEBSITE: &str = "https://allodia.eu";

/// The app icon, 128×128 PNG. Regenerate with `scripts/dev/mcp-icon.py` after a brand change.
const ICON_PNG: &[u8] = include_bytes!("../assets/icon-128.png");

/// The icon as a `data:` URI, encoded once on first use.
///
/// Once rather than per handshake: the encoding is deterministic and the input is a compile-time
/// constant, so re-doing it for every connection would be pure waste; small waste, but on a
/// path that exists to answer a question quickly.
pub fn icon_data_uri() -> &'static str {
    static ENCODED: OnceLock<String> = OnceLock::new();
    ENCODED.get_or_init(|| format!("data:image/png;base64,{}", STANDARD.encode(ICON_PNG)))
}

#[cfg(test)]
mod tests {
    use super::{ICON_PNG, SERVER_TITLE, icon_data_uri};

    /// The eight bytes every PNG starts with.
    const PNG_MAGIC: &[u8] = b"\x89PNG\r\n\x1a\n";

    #[test]
    fn the_icon_is_a_real_png_of_the_declared_size() {
        // The asset is a binary blob produced by a script; if someone replaces it with a JPEG,
        // an SVG, or a truncated file, the declared `image/png` becomes a lie and a
        // spec-conformant client, which is told to "detect content type via magic bytes;
        // reject on mismatch"; will silently show nothing.
        assert!(
            ICON_PNG.starts_with(PNG_MAGIC),
            "assets/icon-128.png is not a PNG",
        );
        // Width and height are big-endian u32s at offsets 16 and 20 of the IHDR chunk.
        let dimension = |at: usize| u32::from_be_bytes(ICON_PNG[at..at + 4].try_into().unwrap());
        assert_eq!((dimension(16), dimension(20)), (128, 128));
    }

    #[test]
    fn the_data_uri_is_well_formed_and_round_trips() {
        use base64::{Engine, engine::general_purpose::STANDARD};

        let uri = icon_data_uri();
        let payload = uri
            .strip_prefix("data:image/png;base64,")
            .expect("the declared media type is part of the URI");
        let decoded = STANDARD.decode(payload).expect("valid base64");
        assert_eq!(decoded, ICON_PNG, "what we send decodes back to the asset");
    }

    #[test]
    fn the_icon_stays_small_enough_to_inline() {
        // Inlining is a wire cost paid on every handshake. A brand refresh that quietly shipped
        // a 400 KB master would make every session slower for no visible gain, and nothing else
        // would notice.
        assert!(
            ICON_PNG.len() < 32 * 1024,
            "the inlined icon is {} bytes; re-run scripts/dev/mcp-icon.py",
            ICON_PNG.len(),
        );
    }

    #[test]
    fn the_display_name_is_the_product_not_the_company() {
        // The brand rule, as a check: "Allodia Mail & Calendar", never bare "Allodia".
        assert_eq!(SERVER_TITLE, "Allodia Mail & Calendar");
    }
}
