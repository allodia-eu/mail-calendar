//! The quote shapes of every catalog locale, generated from `messages/*.json` by `build.rs`.

/// What one locale writes around a quoted original.
pub(crate) struct LocaleShapes {
    /// The catalog locale. Read by the test that holds the language detector to the catalog.
    #[cfg_attr(not(test), expect(dead_code, reason = "read only by a test"))]
    pub(crate) locale: &'static str,
    /// The attribution template: `On {date}, {sender} wrote:`.
    pub(crate) attribution: &'static str,
    /// The line above a forwarded original.
    pub(crate) forwarded: &'static str,
    /// The line-and-header style's labels: from, sent, to, cc, subject.
    pub(crate) headers: &'static [&'static str; 5],
}

/// One entry per catalog locale, in the catalog's order.
pub(crate) const CATALOG: &[LocaleShapes] =
    include!(concat!(env!("OUT_DIR"), "/catalog_shapes.rs"));
