"""Unit tests for `scripts/dev/xctrace_summary.py`.

The summariser fails quietly. A reference it does not resolve reads as an empty backtrace, a thread
it does not recognise drops out of the count, and a waiting sample counted as work makes a grid
that idles look saturated. Each case below pins one of those against a hand-written export in the
shape `xctrace export` writes, compression included.
"""

from __future__ import annotations

import importlib.util
import sys
import tempfile
import unittest
from datetime import datetime, timedelta, timezone
from pathlib import Path

_SPEC = importlib.util.spec_from_file_location(
    "xctrace_summary", Path(__file__).resolve().parents[1] / "xctrace_summary.py"
)
assert _SPEC and _SPEC.loader
summary = importlib.util.module_from_spec(_SPEC)
# Registered before execution because the module defines dataclasses, which resolve their
# annotations through `sys.modules`.
sys.modules["xctrace_summary"] = summary
_SPEC.loader.exec_module(summary)

# Four main-thread samples and one from a worker. The second main-thread sample repeats the first's
# backtrace by reference, the third is the run loop waiting, and the fourth is a different busy
# stack whose app frame refers back to a frame first seen in the first sample.
EXPORT = """<?xml version="1.0"?>
<trace-query-result><node><schema name="time-profile"/>
<row><sample-time id="1" fmt="00:00.100">100000000</sample-time>
  <thread id="2" fmt="Main Thread 0x1 (MailCal, pid: 7)"><process id="3" fmt="MailCal (7)"/></thread>
  <weight id="4" fmt="1.00 ms">1000000</weight>
  <tagged-backtrace id="5">
    <frame id="6" name="CTLineDraw"><binary id="7" name="CoreText"/></frame>
    <frame id="8" name="CalendarWeekContent.draw(_:in:)"><binary id="9" name="MailCal"/></frame>
    <frame id="10" name="main"><binary ref="9"/></frame>
  </tagged-backtrace></row>
<row><sample-time id="11" fmt="00:00.600">600000000</sample-time>
  <thread ref="2"/><weight ref="4"/><tagged-backtrace ref="5"/></row>
<row><sample-time id="12" fmt="00:01.200">1200000000</sample-time>
  <thread ref="2"/><weight ref="4"/>
  <tagged-backtrace id="13">
    <frame id="14" name="mach_msg2_trap"><binary id="15" name="libsystem_kernel.dylib"/></frame>
    <frame ref="10"/>
  </tagged-backtrace></row>
<row><sample-time id="16" fmt="00:01.300">1300000000</sample-time>
  <thread id="17" fmt="tokio-rt-worker 0x2 (MailCal, pid: 7)"><process ref="3"/></thread>
  <weight ref="4"/><tagged-backtrace ref="5"/></row>
<row><sample-time id="18" fmt="00:01.400">1400000000</sample-time>
  <thread ref="2"/><weight ref="4"/>
  <tagged-backtrace id="19">
    <frame id="20" name="objc_msgSend"><binary id="21" name="libobjc.A.dylib"/></frame>
    <frame ref="8"/>
    <frame ref="10"/>
  </tagged-backtrace></row>
<row><sample-time id="22" fmt="00:01.500">1500000000</sample-time><thread ref="2"/><weight ref="4"/></row>
<sentinel/>
</node></trace-query-result>
"""

START = datetime(2026, 11, 8, 11, 9, 2, 440000, tzinfo=timezone(timedelta(hours=1)))


def load(text: str = EXPORT) -> list:
    with tempfile.NamedTemporaryFile("w", suffix=".xml", delete=False) as handle:
        handle.write(text)
    try:
        return summary.load_samples(handle.name)
    finally:
        Path(handle.name).unlink()


class LoadTests(unittest.TestCase):
    def test_keeps_only_the_main_thread_and_skips_a_row_without_a_backtrace(self):
        self.assertEqual([s.seconds for s in load()], [0.1, 0.6, 1.2, 1.4])

    def test_a_referenced_backtrace_reads_as_the_one_it_refers_to(self):
        first, repeat = load()[:2]
        self.assertEqual(repeat.frames, first.frames)
        self.assertEqual(first.frames[0], "CTLineDraw")

    def test_a_referenced_frame_keeps_its_name_and_its_binary(self):
        last = load()[3]
        self.assertEqual(last.frames, ("objc_msgSend", "CalendarWeekContent.draw(_:in:)", "main"))
        self.assertEqual(last.in_app, (False, True, True))

    def test_the_run_loop_waiting_is_not_busy(self):
        self.assertEqual([s.busy for s in load()], [True, True, False, True])

    def test_weight_is_read_through_its_reference(self):
        self.assertEqual({s.weight_ms for s in load()}, {1.0})


class BusyTests(unittest.TestCase):
    def test_busy_time_is_weight_not_a_share_of_samples(self):
        # Three busy samples of 1 ms: 3 ms of work, although three of the four samples are busy.
        self.assertEqual(summary.busy_ms(load()), 3.0)

    def test_per_second_counts_every_second_including_an_idle_one(self):
        self.assertEqual(summary.busy_per_second(load()), {0: 2.0, 1: 1.0})


class TopSymbolTests(unittest.TestCase):
    def test_innermost_frames_are_counted_from_busy_samples_only(self):
        leaf, _ = summary.top_symbols(load(), top=5)
        self.assertEqual(leaf, [("CTLineDraw", 2), ("objc_msgSend", 1)])

    def test_app_frames_count_once_per_sample_and_leave_other_binaries_out(self):
        _, inclusive = summary.top_symbols(load(), top=5)
        self.assertEqual(dict(inclusive), {"CalendarWeekContent.draw(_:in:)": 3, "main": 3})


class PhaseTests(unittest.TestCase):
    LOG = [
        "2026-11-08 11:09:04.000 INFO [apple-ui] calendar frames: 30 in motion at 60 Hz, 0 dropped"
        " (0.0%), gap p50 16.7 p90 16.7 p99 16.7 worst 16.7 ms; inputs: days 0, hours 0, zoom 0;"
        " weeks built 0",
        "2026-11-08 11:09:07.598 INFO [apple-ui] calendar frames: 243 in motion at 60 Hz, 1 dropped"
        " (0.4%), gap p50 16.7 p90 16.7 p99 16.7 worst 33.3 ms; inputs: days 0, hours 778, zoom 0;"
        " weeks built 2",
        "2026-11-08 11:09:12.665 INFO [apple-ui] calendar frames: 237 in motion at 60 Hz, 5 dropped"
        " (2.1%), gap p50 16.7 p90 16.7 p99 33.3 worst 44.2 ms; inputs: days 741, hours 0, zoom 0;"
        " weeks built 6",
        "2026-11-08 11:09:17.732 INFO [apple-ui] calendar frames: 244 in motion at 60 Hz, 0 dropped"
        " (0.0%), gap p50 16.7 p90 16.7 p99 16.7 worst 16.7 ms; inputs: days 0, hours 0, zoom 720;"
        " weeks built 1440",
        "2026-11-08 11:09:18.707 INFO [apple-ui] calendar frames: scripted motion done",
    ]

    def test_each_driven_burst_is_a_phase_named_by_its_inputs(self):
        phases = summary.phases_from_log(self.LOG, START)
        self.assertEqual([p.name for p in phases], ["hours", "days", "zoom"])

    def test_a_phase_ends_when_its_line_was_written_and_spans_the_burst(self):
        hours = summary.phases_from_log(self.LOG, START)[0]
        self.assertAlmostEqual(hours.end, 5.158, places=3)
        self.assertAlmostEqual(hours.end - hours.start, summary.BURST_SECONDS)

    def test_two_lines_on_one_axis_are_one_phase_ending_at_the_second(self):
        split = self.LOG[:2] + [
            "2026-11-08 11:09:08.100 INFO [apple-ui] calendar frames: 20 in motion at 60 Hz,"
            " 0 dropped (0.0%), gap p50 16.7 p90 16.7 p99 16.7 worst 16.7 ms; inputs: days 0,"
            " hours 122, zoom 0; weeks built 0"
        ] + self.LOG[2:]
        phases = summary.phases_from_log(split, START)
        self.assertEqual([p.name for p in phases], ["hours", "days", "zoom"])
        self.assertAlmostEqual(phases[0].end, 5.66, places=2)

    def test_the_start_date_is_read_from_the_table_of_contents(self):
        toc = "<summary><start-date>2026-11-08T11:09:02.440+01:00</start-date></summary>"
        self.assertEqual(summary.trace_start(toc), START)


if __name__ == "__main__":
    unittest.main()
