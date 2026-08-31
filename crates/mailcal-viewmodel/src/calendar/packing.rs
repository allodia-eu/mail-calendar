//! The overlap solver: assigning side-by-side columns to intervals that collide.
//!
//! Pure interval arithmetic: no dates, no zones, no pixels. A caller hands it
//! half-open `[start, end)` spans in whatever unit it likes (the time grid uses
//! minutes from the top of a day column) and gets back, for each one, which column it
//! sits in and how many columns its neighbourhood was split into. The caller
//! multiplies by its own width.
//!
//! This lives in Rust, once, because three native clients must place the same
//! overlapping meetings in the same columns. Two implementations of a greedy packer
//! disagree on the interesting cases almost immediately, and the disagreement is
//! invisible until someone compares two screens.

/// A half-open interval `[start, end)` to place.
///
/// Empty (`start == end`) intervals overlap nothing (not even each other) which is
/// mathematically right and visually wrong. A caller that renders zero-length events
/// gives them a minimum span *before* packing (the time grid does); this module keeps
/// to interval semantics and does not guess a display minimum.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    /// The inclusive lower bound.
    pub start: i64,
    /// The exclusive upper bound.
    pub end: i64,
}

impl Span {
    /// Creates a span, clamping an inverted interval to empty at `start`.
    #[must_use]
    pub fn new(start: i64, end: i64) -> Self {
        Self {
            start,
            end: end.max(start),
        }
    }

    /// Whether this span and `other` share any instant.
    #[must_use]
    pub fn overlaps(self, other: Self) -> bool {
        self.start < other.end && other.start < self.end
    }
}

/// Where one span sits: its column, and how many columns its cluster was split into.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Placement {
    /// The 0-based column this span occupies.
    pub column: u32,
    /// How many columns the span's cluster needs: the divisor for its width. Always
    /// at least 1, and always greater than [`Self::column`].
    pub columns: u32,
}

/// Packs `spans` into columns, returning one [`Placement`] per input **in input order**.
///
/// Transitively-overlapping spans form a *cluster* and share a column count, so a
/// cluster renders as an even set of side-by-side lanes rather than a ragged staircase.
/// Within a cluster each span takes the first column free at its start: the greedy
/// choice every calendar makes; scanning spans in `(start, end, input index)` order.
///
/// That ordering is the load-bearing detail: it is a **total** order over the input, so
/// the result cannot depend on the order the caller happened to collect the spans in.
/// Sort by start alone and two hosts reading the same events from different providers
/// can column them differently.
#[must_use]
pub fn pack(spans: &[Span]) -> Vec<Placement> {
    let mut order: Vec<usize> = (0..spans.len()).collect();
    order.sort_by_key(|&i| (spans[i].start, spans[i].end, i));

    let mut placements = vec![
        Placement {
            column: 0,
            columns: 1
        };
        spans.len()
    ];
    // The spans of the cluster being built, and the last end in each of its columns.
    let mut cluster: Vec<usize> = Vec::new();
    let mut column_ends: Vec<i64> = Vec::new();
    let mut cluster_end = i64::MIN;

    for &i in &order {
        let span = spans[i];
        // A span starting at or after every current column's end begins a new cluster:
        // nothing in the old one can reach it, so their widths are independent.
        if !cluster.is_empty() && span.start >= cluster_end {
            finish(&cluster, column_ends.len(), &mut placements);
            cluster.clear();
            column_ends.clear();
            cluster_end = i64::MIN;
        }
        // The first column whose last span has ended; otherwise a new column.
        let column = column_ends
            .iter()
            .position(|&end| end <= span.start)
            .unwrap_or_else(|| {
                column_ends.push(i64::MIN);
                column_ends.len() - 1
            });
        column_ends[column] = span.end;
        placements[i].column = u32::try_from(column).unwrap_or(u32::MAX);
        cluster.push(i);
        cluster_end = cluster_end.max(span.end);
    }
    finish(&cluster, column_ends.len(), &mut placements);
    placements
}

/// Stamps the finished cluster's column count onto each of its members.
fn finish(cluster: &[usize], columns: usize, placements: &mut [Placement]) {
    let columns = u32::try_from(columns.max(1)).unwrap_or(u32::MAX);
    for &i in cluster {
        placements[i].columns = columns;
    }
}

#[cfg(test)]
#[path = "packing_tests.rs"]
mod packing_tests;
