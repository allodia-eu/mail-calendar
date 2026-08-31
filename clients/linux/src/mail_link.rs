//! Desktop mail-link ingress. Decoding and header policy stay in the shared core.

use std::ffi::OsString;

use mailcal_bindings::{MailtoPrefill, parse_mailto_uri};

fn prefill(uri: &str) -> Option<MailtoPrefill> {
    parse_mailto_uri(uri.to_owned())
}

pub(crate) fn prefill_arguments(arguments: &[OsString]) -> Option<MailtoPrefill> {
    arguments
        .iter()
        .skip(1)
        .find_map(|argument| argument.to_str().and_then(prefill))
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;

    use super::{prefill, prefill_arguments};

    #[test]
    fn only_mail_links_reach_the_shell() {
        assert!(prefill("https://allodia.eu").is_none());
        let mail = prefill("mailto:ada@example.test?subject=Hello").expect("mail link");
        assert_eq!(mail.to, "ada@example.test");
        assert_eq!(mail.subject, "Hello");
    }

    #[test]
    fn command_line_keeps_the_opaque_mailto_recipient_exact() {
        let arguments = [
            OsString::from("mailcal-linux"),
            OsString::from("mailto:ada@example.test?subject=Hello"),
        ];
        let mail = prefill_arguments(&arguments).expect("mail link");
        assert_eq!(mail.to, "ada@example.test");
    }
}
