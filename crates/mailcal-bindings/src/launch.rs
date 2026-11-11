//! What a host shows while the core it asked for is still opening.

/// How long, in milliseconds, a host lets [`crate::MailcalApp::new_accounts`] run before its launch
/// view says the mailbox is being opened (`docs/boot-sequence.md`). Until then the window is a
/// blank page: a normal open takes up to about a second on a desktop, and a label raised and
/// removed within it reads as flicker rather than as a fast launch. A store migration after an
/// update takes longer, and that is the wait this is for.
#[uniffi::export]
#[must_use]
pub fn launch_status_after_ms() -> u32 {
    2000
}

/// Whether boot can read a stored account `config`: the same parse and id derivation
/// [`crate::MailcalApp::new_accounts`] runs, with no token source built and nothing registered.
///
/// A host asks before it removes an entry it believes is beyond recovery, so that only a config the
/// core itself leaves out is ever removed.
#[uniffi::export]
#[must_use]
pub fn stored_account_is_readable(config: String) -> bool {
    crate::boot::StoredConfig::parse(&config).is_ok()
}

#[cfg(test)]
mod tests {
    use super::stored_account_is_readable;

    #[test]
    fn only_a_config_boot_can_open_is_readable() {
        let config = mailcal_account::JmapAccountConfig {
            email: "reader@example.com".to_owned(),
            base_url: "https://jmap.example.com".to_owned(),
            password: Some(mailcal_account::Secret::new("pw".to_owned())),
            token: None,
            oauth: None,
            shape: mailcal_account::AccountShape::default(),
        }
        .to_toml()
        .expect("serializable config");
        let first_line = config.lines().next().expect("a first line").to_owned();

        assert!(stored_account_is_readable(config));
        assert!(!stored_account_is_readable(first_line));
        assert!(!stored_account_is_readable(String::new()));
    }
}
