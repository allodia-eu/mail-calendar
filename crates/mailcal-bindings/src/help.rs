//! The help pages a client links to, decided here so every client opens the same page.

/// Where the help pages are published.
const HELP_SITE: &str = "https://allodia.eu";

/// The locales the help pages are written in besides English, each under a path of its own. A
/// reader in any other locale is sent to the English page rather than to one that does not exist.
const HELP_LOCALES: &[&str] = &["nl"];

/// A help page a client links to.
#[derive(uniffi::Enum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum HelpPage {
    /// Which way a Gmail account syncs, and how to get the faster one.
    OptimiseGmailSync,
}

impl HelpPage {
    const fn slug(self) -> &'static str {
        match self {
            Self::OptimiseGmailSync => "optimise-gmail-sync-performance",
        }
    }
}

/// `page`'s address for a reader in `locale` (the client's active catalog locale, `nl` or
/// `nl-NL` alike): the page in that language where there is one, the English page otherwise.
#[uniffi::export]
#[must_use]
pub fn help_page_url(page: HelpPage, locale: String) -> String {
    let language = locale
        .split(['-', '_'])
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    let prefix = if HELP_LOCALES.contains(&language.as_str()) {
        format!("/{language}")
    } else {
        String::new()
    };
    format!("{HELP_SITE}{prefix}/docs/mail-calendar/{}", page.slug())
}

#[cfg(test)]
mod tests {
    use super::{HelpPage, help_page_url};

    #[test]
    fn a_reader_gets_the_page_in_their_language_where_it_is_written() {
        let url = |locale: &str| help_page_url(HelpPage::OptimiseGmailSync, locale.to_owned());
        assert_eq!(
            url("en"),
            "https://allodia.eu/docs/mail-calendar/optimise-gmail-sync-performance"
        );
        assert_eq!(
            url("nl"),
            "https://allodia.eu/nl/docs/mail-calendar/optimise-gmail-sync-performance"
        );
        assert_eq!(url("nl-BE"), url("nl"));
        assert_eq!(url("NL_nl"), url("nl"));
        // German readers get the English page: the help site writes none in German.
        assert_eq!(url("de"), url("en"));
        assert_eq!(url(""), url("en"));
    }
}
