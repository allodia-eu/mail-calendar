//! What the autosave makes of the editor's change count (`docs/drafts.md`, "When a save
//! happens").
//!
//! The baseline is the count the seeds left, read once they are in, never a sample: one read
//! before the seeds counts the quote and the signature as an edit, so a composer nobody touched
//! would be saved, and one read on a later sample takes in whatever was typed first, so a short
//! reply written straight after opening would not be.

/// The editor's change count as the autosave knows it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum EditorRevision {
    /// The seeds are still going in, so no reading counts.
    #[default]
    Seeding,
    /// Seeded, but the count could not be read then: the first reading is the baseline.
    Unknown,
    /// The count the seeds left, or the last one read since.
    Seen(i32),
}

impl EditorRevision {
    /// What the seeding left: the count read once the seeds were in, `None` when it could not be
    /// read.
    pub(crate) fn seeded(count: Option<i32>) -> Self {
        count
            .filter(|count| *count >= 0)
            .map_or(Self::Unknown, Self::Seen)
    }

    /// Takes in one reading and answers whether the message changed since the last one.
    pub(crate) fn changed(&mut self, now: i32) -> bool {
        match *self {
            Self::Seeding => false,
            Self::Unknown => {
                *self = Self::Seen(now);
                false
            }
            Self::Seen(seen) => {
                *self = Self::Seen(now);
                seen != now
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::EditorRevision;

    #[test]
    fn nothing_counts_while_the_seeds_are_going_in() {
        let mut revision = EditorRevision::default();
        assert!(!revision.changed(4));
        assert_eq!(revision, EditorRevision::Seeding);
    }

    /// The quote and the signature moved the count before the first sample: an untouched
    /// composer reads as untouched.
    #[test]
    fn the_seeds_are_the_baseline_not_an_edit() {
        let mut revision = EditorRevision::seeded(Some(4));
        assert!(!revision.changed(4));
        assert!(!revision.changed(4));
    }

    /// Typed before the first sample: that sample is the edit, so a short reply is saved.
    #[test]
    fn typing_before_the_first_sample_is_an_edit() {
        let mut revision = EditorRevision::seeded(Some(4));
        assert!(revision.changed(7));
        assert!(!revision.changed(7));
        assert!(revision.changed(9));
    }

    #[test]
    fn a_baseline_that_could_not_be_read_is_the_first_reading() {
        for unreadable in [None, Some(-1)] {
            let mut revision = EditorRevision::seeded(unreadable);
            assert!(!revision.changed(6));
            assert!(revision.changed(8));
        }
    }
}
