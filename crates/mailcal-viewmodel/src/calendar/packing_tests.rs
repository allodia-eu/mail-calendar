//! Packing cases, plus the invariants that must hold for *any* input.
//!
//! The invariant tests run over pseudo-random span sets from a seeded generator, so
//! they explore hundreds of shapes while staying byte-reproducible on every machine;
//! a failure prints the seed and replays exactly.

use super::*;

fn spans(raw: &[(i64, i64)]) -> Vec<Span> {
    raw.iter().map(|&(s, e)| Span::new(s, e)).collect()
}

/// `(column, columns)` per input span, for terse assertions.
fn packed(raw: &[(i64, i64)]) -> Vec<(u32, u32)> {
    pack(&spans(raw))
        .iter()
        .map(|p| (p.column, p.columns))
        .collect()
}

#[test]
fn disjoint_spans_each_take_the_whole_width() {
    // Back-to-back meetings do NOT overlap: 10:00–11:00 and 11:00–12:00 share only the
    // instant 11:00, which the half-open interval excludes. Splitting the day into two
    // columns for these would be the classic off-by-one, and it looks obviously wrong.
    assert_eq!(packed(&[(600, 660), (660, 720)]), vec![(0, 1), (0, 1)]);
    // A gap between them, likewise.
    assert_eq!(packed(&[(600, 660), (900, 960)]), vec![(0, 1), (0, 1)]);
}

#[test]
fn two_overlapping_spans_split_the_width() {
    assert_eq!(packed(&[(600, 720), (660, 780)]), vec![(0, 2), (1, 2)]);
}

#[test]
fn a_cluster_shares_one_column_count_even_where_it_is_thinner() {
    // A ──────────────         (600..900)
    //   B ────────             (660..780)
    //          C ────────      (780..900)
    // B and C do not overlap each other, so they share column 1, but all three are one
    // transitively-overlapping cluster (A touches both), so all three are half-width.
    // A ragged cluster where C jumped back to full width would tear the layout.
    assert_eq!(
        packed(&[(600, 900), (660, 780), (780, 900)]),
        vec![(0, 2), (1, 2), (1, 2)]
    );
}

#[test]
fn a_free_column_is_reused_before_a_new_one_is_opened() {
    // Three mutually-overlapping spans need three columns...
    assert_eq!(
        packed(&[(600, 700), (610, 700), (620, 700)]),
        vec![(0, 3), (1, 3), (2, 3)]
    );
    // ...but a fourth starting after the first two have ended reuses column 0 rather
    // than opening a fourth, so the cluster stays three wide.
    assert_eq!(
        packed(&[(600, 660), (610, 660), (620, 700), (660, 700)]),
        vec![(0, 3), (1, 3), (2, 3), (0, 3)]
    );
}

#[test]
fn clusters_are_independent() {
    // Two separate pile-ups in a day do not force each other wider: the morning's
    // three-way clash must not squeeze the afternoon's lone meeting to a third width.
    assert_eq!(
        packed(&[(540, 600), (550, 600), (560, 600), (900, 960)]),
        vec![(0, 3), (1, 3), (2, 3), (0, 1)]
    );
}

#[test]
fn a_span_covering_the_whole_day_clusters_with_everything_it_covers() {
    assert_eq!(
        packed(&[(0, 1440), (600, 660), (900, 960)]),
        vec![(0, 2), (1, 2), (1, 2)]
    );
}

#[test]
fn placements_come_back_in_input_order_not_sorted_order() {
    // The caller zips these against its own rows, so the mapping must be positional.
    // Input is deliberately unsorted: the later-starting span is first.
    assert_eq!(packed(&[(660, 780), (600, 720)]), vec![(1, 2), (0, 2)]);
}

#[test]
fn the_result_does_not_depend_on_the_input_order() {
    // The load-bearing property. Two hosts reading the same events from different
    // providers collect them in different orders; if that changed the columns, the same
    // calendar would render differently on a phone and a laptop.
    let raw = [(600, 720), (660, 780), (700, 730), (900, 960), (0, 1440)];
    let forward = pack(&spans(&raw));

    let mut reversed: Vec<Span> = spans(&raw);
    reversed.reverse();
    let mut back = pack(&reversed);
    back.reverse();
    assert_eq!(forward, back);

    // And ties (identical spans) are broken deterministically, not arbitrarily.
    let identical = spans(&[(600, 660), (600, 660), (600, 660)]);
    assert_eq!(
        pack(&identical)
            .iter()
            .map(|p| p.column)
            .collect::<Vec<_>>(),
        vec![0, 1, 2]
    );
}

#[test]
fn an_empty_span_overlaps_nothing() {
    // `Span` is half-open, so a zero-length interval is empty and collides with nobody;
    // including another at the same instant. The time grid gives a zero-length event a
    // display minimum *before* packing; this module keeps to interval semantics.
    assert_eq!(packed(&[(600, 600), (600, 600)]), vec![(0, 1), (0, 1)]);
    assert!(!Span::new(600, 600).overlaps(Span::new(600, 600)));
    // An inverted span is clamped to empty rather than packing as a negative width.
    assert_eq!(Span::new(700, 600), Span::new(700, 700));
}

#[test]
fn no_spans_is_no_placements() {
    assert!(pack(&[]).is_empty());
}

// --- invariants over generated input ------------------------------------------------

/// A tiny reproducible LCG, so the invariant sweep explores a lot of shapes without a
/// new dependency and without flaking: same seed, same spans, on every machine.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1);
        self.0 >> 33
    }

    fn below(&mut self, n: u64) -> i64 {
        i64::try_from(self.next() % n).expect("in range")
    }
}

/// Spans over a day, biased to collide: short durations on a coarse start grid, so
/// pile-ups and exact ties are common rather than vanishingly rare.
fn generated(seed: u64, count: usize) -> Vec<Span> {
    let mut rng = Rng(seed);
    (0..count)
        .map(|_| {
            let start = rng.below(24) * 60 + rng.below(4) * 15;
            let end = start + rng.below(180);
            Span::new(start, end)
        })
        .collect()
}

#[test]
fn overlapping_spans_never_share_a_column() {
    // The one invariant a user sees violated instantly: two meetings drawn on top of
    // each other.
    for seed in 0..200 {
        let spans = generated(seed, 12);
        let placed = pack(&spans);
        for (i, a) in spans.iter().enumerate() {
            for (j, b) in spans.iter().enumerate().skip(i + 1) {
                if a.overlaps(*b) {
                    assert_ne!(
                        placed[i].column, placed[j].column,
                        "seed {seed}: overlapping spans {a:?} and {b:?} both in column {}",
                        placed[i].column
                    );
                }
            }
        }
    }
}

#[test]
fn every_column_is_within_its_span_s_column_count() {
    // A `column >= columns` renders off the right edge of the day, or divides by a width
    // it was never given.
    for seed in 0..200 {
        let spans = generated(seed, 12);
        for (span, p) in spans.iter().zip(pack(&spans)) {
            assert!(
                p.column < p.columns,
                "seed {seed}: {span:?} got column {} of {}",
                p.column,
                p.columns
            );
            assert!(p.columns >= 1);
        }
    }
}

#[test]
fn the_column_count_is_wide_enough_for_the_worst_pile_up_it_covers() {
    // A span must have at least as many columns as there are spans live at any one
    // *instant* it covers; otherwise two of them are forced to share a lane and overlap.
    //
    // Note this is concurrency at a point in time, NOT the number of spans that touch this
    // one. Those differ: if A covers both B and C but B and C are disjoint, A touches two
    // others yet only two columns are ever needed (B and C share one). Asserting on the
    // touch count would demand three, and fail a correct packer.
    for seed in 0..200 {
        let spans = generated(seed, 12);
        let placed = pack(&spans);
        for (i, span) in spans.iter().enumerate() {
            if span.start == span.end {
                continue; // empty: overlaps nothing, so there is no clique to cover
            }
            // Every span's start is a candidate instant; the peak always occurs at one.
            let peak = spans
                .iter()
                .filter(|other| span.overlaps(**other))
                .map(|probe| {
                    spans
                        .iter()
                        .filter(|other| other.start <= probe.start && probe.start < other.end)
                        .count()
                })
                .max()
                .unwrap_or(1);
            let peak = u32::try_from(peak).expect("small");
            assert!(
                placed[i].columns >= peak,
                "seed {seed}: {span:?} covers an instant with {peak} concurrent spans but \
                 got only {} columns",
                placed[i].columns
            );
        }
    }
}

#[test]
fn packing_is_stable_under_permutation_for_generated_input() {
    for seed in 0..200 {
        let spans = generated(seed, 10);
        let straight = pack(&spans);

        // Rotate the input; the placements must follow their spans, unchanged.
        let mut rotated = spans.clone();
        rotated.rotate_left(3);
        let rotated_placed = pack(&rotated);
        for (i, placement) in straight.iter().enumerate() {
            let moved = (i + spans.len() - 3) % spans.len();
            assert_eq!(
                *placement, rotated_placed[moved],
                "seed {seed}: span {i} moved column when the input was rotated"
            );
        }
    }
}
