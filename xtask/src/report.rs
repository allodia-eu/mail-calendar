//! Collected findings for a check that reports everything wrong rather than the first thing.
//!
//! A check that stops at its first note sends someone round the loop once per problem. These rules
//! are mostly independent: a stale mirror and a missing release note have nothing to do with each
//! other, so they are all gathered and printed together.

/// Notes gathered while a check runs.
#[derive(Debug, Default)]
pub(crate) struct Report {
    notes: Vec<String>,
}

impl Report {
    /// An empty report.
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Records one finding.
    pub(crate) fn note(&mut self, what: impl Into<String>) {
        self.notes.push(what.into());
    }

    /// True when anything was recorded.
    pub(crate) fn failed(&self) -> bool {
        !self.notes.is_empty()
    }

    /// Prints the findings, indented, to stderr.
    pub(crate) fn emit(&self) {
        for note in &self.notes {
            eprintln!("  {note}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Report;

    #[test]
    fn an_empty_report_has_not_failed() {
        assert!(!Report::new().failed());
    }

    #[test]
    fn a_note_fails_the_report() {
        let mut report = Report::new();
        report.note("something");
        assert!(report.failed());
    }
}
