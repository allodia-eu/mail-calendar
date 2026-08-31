"""Unit tests for `scripts/dev/timings.py`.

The reducer's failure mode is silence. A pattern that stops matching does not raise; the
operation simply vanishes from the table, and a table with a missing row reads exactly like
an operation that never ran. So the cases below pin the two things that would let that
happen: every timing line the core emits is claimed by some pattern, and a line that carries
a duration for something else is *deliberately* ignored rather than accidentally counted.

The percentile cases matter for a second reason: the whole point of the table is the tail,
and an off-by-one in the rank is invisible until someone acts on a p99 that is really a p95.
"""

from __future__ import annotations

import importlib.util
import sys
import unittest
from pathlib import Path

_SPEC = importlib.util.spec_from_file_location(
    "timings", Path(__file__).resolve().parents[1] / "timings.py"
)
assert _SPEC and _SPEC.loader
timings = importlib.util.module_from_spec(_SPEC)
# Registered before execution because the module defines a dataclass, which resolves its
# annotations through `sys.modules`.
sys.modules["timings"] = timings
_SPEC.loader.exec_module(timings)


def record(body: str, stamp: str = "2026-08-12") -> str:
    """Wraps a message body in the log record format every client writes."""
    return f"{stamp} 16:00:08.517 INFO [mailcal_app::snapshot] {body}"


class PercentileTests(unittest.TestCase):
    def test_nearest_rank_reports_an_observed_value(self):
        ascending = list(range(1, 101))
        self.assertEqual(timings.percentile(ascending, 50), 50)
        self.assertEqual(timings.percentile(ascending, 90), 90)
        self.assertEqual(timings.percentile(ascending, 99), 99)

    def test_a_short_series_still_has_every_percentile(self):
        self.assertEqual(timings.percentile([7], 99), 7)
        self.assertEqual(timings.percentile([1, 2, 3], 50), 2)
        self.assertEqual(timings.percentile([], 50), 0)

    def test_the_tail_survives_a_fast_median(self):
        # The shape the table exists to show: a mailbox that is quick almost always and
        # occasionally stalls for fifteen seconds. A mean would hide it.
        samples = sorted([20] * 99 + [14_763])
        self.assertEqual(timings.percentile(samples, 50), 20)
        self.assertEqual(samples[-1], 14_763)


class ClassifyTests(unittest.TestCase):
    def test_every_line_the_core_emits_is_claimed(self):
        # One example of each timing line in the product core, taken from its format
        # strings. A line added there without a pattern here is a row that never appears.
        emitted = {
            "rebuild_snapshot: 100 row(s) of 640 in 479ms": [("rebuild_snapshot", 479)],
            "refresh_mail: sync 12ms + rebuild 34ms": [
                ("refresh_mail/sync", 12),
                ("refresh_mail/rebuild", 34),
            ],
            "sync[a0]: 6 folder(s), 12 msg upserted in 900ms": [("sync_account", 900)],
            "sync[a0]: 6 folder(s), 12 msg upserted, 1 busy scope(s) in 900ms": [
                ("sync_account", 900)
            ],
            "sync[a0]: folder[2] +5 -1 in 72ms": [("sync/folder", 72)],
            "sync[a0]: folder list 40ms": [("sync/folder_list", 40)],
            "sync[a0]: derive threads 163ms": [("sync/derive_threads", 163)],
            "cached_messages: loaded + deserialized 146 message(s) at window 500 in 13ms "
            "(cache miss)": [("cached_messages", 13)],
            "thread completion: 30 thread(s) -> 4 out-of-window member(s) in 9ms": [
                ("thread_completion", 9)
            ],
            "on-demand: folder synced in 271ms": [("folder_on_demand", 271)],
            "prefetch: warmed 12/40 bodies in 189ms": [("prefetch_bodies", 189)],
            "rebuild_calendar_cache: 88 occurrence(s), 3 calendar(s) in 168ms": [
                ("rebuild_calendar_cache", 168)
            ],
            "rebuild_calendar_cache: 88 occurrence(s), 3 calendar(s) in 168ms; unchanged": [
                ("rebuild_calendar_cache", 168)
            ],
            "refresh_calendar: sync+expand 6ms + rebuild 47ms": [
                ("refresh_calendar/sync", 6),
                ("refresh_calendar/rebuild", 47),
            ],
            "refresh_calendar: sync+expand 6ms + rebuild 47ms; no redraw": [
                ("refresh_calendar/sync", 6),
                ("refresh_calendar/rebuild", 47),
            ],
            "rebuild_contacts: 40 row(s), query_chars=0 in 2ms": [("rebuild_contacts", 2)],
            "boot: engine open+migrate in 6ms": [("boot/open+migrate", 6)],
            "boot: abandoned 0 interrupted sync scope lease(s) in 11ms": [
                ("boot/abandon_leases", 11)
            ],
            "boot: primed cached snapshot in 420ms; NewAccounts total 800ms": [
                ("boot/prime_snapshot", 420),
                ("boot/new_accounts", 800),
            ],
            "event_detail: resolved in 849ms": [("event_detail", 849)],
        }
        for body, expected in emitted.items():
            with self.subTest(body=body):
                self.assertEqual(sorted(timings.classify(body)), sorted(expected))

    def test_a_qualified_line_is_not_dropped(self):
        # `; unchanged` and `(cache miss)` mark the runs most worth looking at, so a
        # pattern anchored hard at the millisecond would lose exactly the wrong samples.
        self.assertTrue(
            timings.classify("rebuild_calendar_cache: 1 occurrence(s), 1 calendar(s) in 4ms;"
                             " unchanged")
        )
        self.assertTrue(
            timings.classify("cached_messages: loaded + deserialized 3 message(s) at "
                             "window 500 in 0ms (cache miss)")
        )


class IgnoredTests(unittest.TestCase):
    def test_network_and_client_round_trips_are_not_engine_work(self):
        for body in [
            "oauth: google: refreshed in 300ms",
            "carddav: bound 2 contact source(s) in 500ms",
            "mcp: tools/call list_messages -> ok in 12ms",
            "connect[imap]: authenticated",
            "boot: account[0] connect ok in 900ms",
            "reconnect: account refresh hit 1 busy scope(s); retrying in 250ms",
            "on-demand: connected folder in 1200ms",
        ]:
            with self.subTest(body=body):
                self.assertEqual(timings.classify(body), [])
                self.assertTrue(timings.IGNORED.search(body), "should be ignored, not unmatched")

    def test_a_failed_or_contended_scope_is_not_counted_as_throughput(self):
        # These carry a real duration, and counting them would make a sync that failed
        # after 12ms look like the fastest sync of the day.
        for body in [
            "sync[a0]: folder[0] failed in 12ms: sync error: provider error",
            "sync[a0]: folder list failed in 48ms: sync error",
            "sync[a0]: folder[0] busy in 0ms",
        ]:
            with self.subTest(body=body):
                self.assertEqual(timings.classify(body), [])
                self.assertTrue(timings.IGNORED.search(body))


class CollectTests(unittest.TestCase):
    def test_only_records_in_range_are_counted(self):
        lines = [
            record("rebuild_snapshot: 1 row(s) of 1 in 10ms", "2026-08-11"),
            record("rebuild_snapshot: 1 row(s) of 1 in 20ms", "2026-08-12"),
            record("rebuild_snapshot: 1 row(s) of 1 in 30ms", "2026-08-13"),
        ]
        samples, _ = timings.collect(lines, since="2026-08-12", until="2026-08-12")
        self.assertEqual(samples["rebuild_snapshot"].durations, [20])

        samples, _ = timings.collect(lines)
        self.assertEqual(samples["rebuild_snapshot"].durations, [10, 20, 30])

    def test_a_line_that_is_not_a_log_record_is_skipped(self):
        # A pasted log routinely picks up a shell prompt or a wrapped stack frame.
        samples, unmatched = timings.collect(["$ tail -f mailcal.log", "", "  at frame 3"])
        self.assertEqual(samples, {})
        self.assertEqual(unmatched, [])

    def test_an_unmapped_timing_line_is_reported_rather_than_swallowed(self):
        # The one thing that must never be silent: the core renamed a line, so the row it
        # fed has quietly disappeared from the table.
        _, unmatched = timings.collect([record("sync[a0]: mailbox list 5ms")])
        self.assertEqual(unmatched, ["sync[a0]: mailbox list 5ms"])

    def test_the_worst_run_keeps_the_line_it_came_from(self):
        lines = [
            record("rebuild_snapshot: 1 row(s) of 1 in 10ms"),
            record("rebuild_snapshot: 1 row(s) of 1 in 900ms"),
        ]
        samples, _ = timings.collect(lines)
        worst = max(samples["rebuild_snapshot"].worst)
        self.assertEqual(worst[0], 900)
        self.assertIn("900ms", worst[1])


class RenderTests(unittest.TestCase):
    def test_the_table_has_the_columns_the_engine_benchmarks_print(self):
        samples, _ = timings.collect(
            [record(f"rebuild_snapshot: 1 row(s) of 1 in {ms}ms") for ms in range(1, 101)]
        )
        table = timings.render(samples, "mailcal.log")
        lines = [line for line in table.splitlines() if line.startswith("|")]
        header, _separator, row = lines
        for column in ("Operation", "n", "p50", "p90", "p99", "max"):
            self.assertIn(column, header)
        self.assertIn("rebuild_snapshot", row)
        self.assertIn("100", row)
        self.assertIn("50 ms", row)
        self.assertIn("99 ms", row)

    def test_the_slowest_runs_name_when_they_happened(self):
        samples, _ = timings.collect(
            [
                record("rebuild_snapshot: 1 row(s) of 1 in 10ms", "2026-08-11"),
                record("rebuild_snapshot: 1 row(s) of 1 in 900ms", "2026-08-12"),
            ]
        )
        worst = timings.render_worst(samples, top=1)
        self.assertIn("900 ms", worst)
        self.assertIn("2026-08-12", worst)
        self.assertNotIn("2026-08-11", worst)


if __name__ == "__main__":
    unittest.main()
