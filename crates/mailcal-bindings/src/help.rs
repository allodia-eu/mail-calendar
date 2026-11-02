//! The help pages a client links to, decided here so every client opens the same page.

/// Where the help pages are published. The site sends a reader on to the page in their own
/// language (their browser's, or the one they chose there), so a client links one address for
/// everyone.
const HELP_PAGES: &str = "https://allodia.eu/docs/mail-calendar";

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

/// `page`'s address.
#[uniffi::export]
#[must_use]
pub fn help_page_url(page: HelpPage) -> String {
    format!("{HELP_PAGES}/{}", page.slug())
}

#[cfg(test)]
mod tests {
    use super::{HelpPage, help_page_url};

    #[test]
    fn the_gmail_sync_page_has_one_address_for_every_reader() {
        assert_eq!(
            help_page_url(HelpPage::OptimiseGmailSync),
            "https://allodia.eu/docs/mail-calendar/optimise-gmail-sync-performance"
        );
    }
}
