//! The `User-Agent` this application sends with every HTTP request it makes, through the engine's
//! adapters and its own (RFC 9110 §10.1.5): the product the brand names, and the version. It is
//! what lets a provider tell this application's traffic apart from anyone else's.

/// The product token, derived from the brand's name when this crate is built: "Allodia Mail &
/// Calendar" is `allodia-mail-calendar`, "MailCal" is `mailcal`.
pub const PRODUCT: &str = env!("MAILCAL_USER_AGENT_PRODUCT");

/// The application's version ([`VERSION`](https://github.com/allodia-eu/mail-calendar/blob/main/VERSION)).
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The header's value: `PRODUCT/VERSION`.
pub const HEADER: &str = concat!(
    env!("MAILCAL_USER_AGENT_PRODUCT"),
    "/",
    env!("CARGO_PKG_VERSION")
);

#[cfg(test)]
mod tests {
    use super::{HEADER, PRODUCT, VERSION};

    #[test]
    fn the_header_is_the_product_and_the_version_both_tokens() {
        assert_eq!(HEADER, format!("{PRODUCT}/{VERSION}"));
        for token in [PRODUCT, VERSION] {
            assert_ne!(token, "");
            assert!(
                token
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || "!#$%&'*+-.^_`|~".contains(c)),
                "{token} is not an RFC 9110 token"
            );
        }
    }
}
