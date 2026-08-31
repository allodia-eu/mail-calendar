// What the grid did, in numbers, the instrument for the bugs that only exist in a hand.
//
// It went in as temporary scaffolding and stayed, because it found two things nothing else could.
// A pinch frame costing 3.4x a swipe frame *while drawing half the blocks* is not "the pinch is
// heavy", cost going up as work goes down is a clue, and it pointed straight at the text shaper
// missing its cache on every frame of a zoom. And logging what the single owner DECIDED each finger
// was (`pan_x` / `pan_y` / `zoom`) is the only way to prove a real hand's pinch is not being misread
// as a pan: `adb` cannot inject two fingers, so no script will ever tell you.
//
// **This logs counts and durations. It never logs an event.** No title, no time, no attendee, no
// calendar name, the developer validating this has their own mail and their own diary on the device,
// and docs/logging.md's never-log-content rule is absolute regardless of how convenient a title would
// be in a trace. What a frame *costs* is a number; what is *in* it is nobody's business.
//
// Off unless asked for, in **any** build including release, which is the point, because the only
// build worth judging the grid on is a release one (an unminified Compose build is several times
// slower, and that is exactly how the old grid came to be measured against Samsung's release build
// while running as a debug one). So the switch is a log tag rather than a debug flag, and the cost of
// being switched off is one cached boolean:
//
//     adb shell setprop log.tag.MailcalCal DEBUG     # then launch
//     adb logcat -s MailcalCal
//
// Read once, into a `val`: the per-frame cost of being switched off is one boolean.
package eu.allodia.mailcal

import android.util.Log

private const val TAG = "MailcalCal"

/**
 * What the grid did, in numbers, flushed once a second.
 *
 * Deliberately not a `Logger` port call per event: the point is to measure a frame, and a trace that
 * writes to disk inside the frame it is measuring is measuring itself.
 */
internal object CalendarTrace {
    /** Cached at class-load. `setprop` before launching, not after. */
    @JvmField val on: Boolean = Log.isLoggable(TAG, Log.DEBUG)

    private var frames = 0
    private var blocksDrawn = 0
    private var blocksCulled = 0
    private var measures = 0
    private var drawNanos = 0L
    private var worstDrawNanos = 0L
    private var pagesPainted = 0
    private var panDays = 0
    private var panHours = 0
    private var zooms = 0
    private var taps = 0
    private var turns = 0
    private var lastFlush = 0L

    /** One page, drawn. [culled] is what the viewport saved us, the whole point of drawing by hand. */
    fun frame(drawn: Int, culled: Int, nanos: Long) {
        if (!on) return
        frames++
        blocksDrawn += drawn
        blocksCulled += culled
        drawNanos += nanos
        if (nanos > worstDrawNanos) worstDrawNanos = nanos
        flush(System.nanoTime())
    }

    /** A line of text shaped. A pinch that misses the shaper's cache does this on every frame. */
    fun measured() {
        if (on) measures++
    }

    /** A week turned into a [PagePaint], should be *one* per swipe, not three. */
    fun painted() {
        if (on) pagesPainted++
    }

    /**
     * What the single gesture owner decided this finger was.
     *
     * Flushes on its own clock as well as the frame's. It used to flush only from [frame], and so a
     * burst of flicks that ended with the grid going idle left its last counters in limbo, the log
     * reported six of eight gestures, and the two it swallowed were its own, not the app's. An
     * instrument that under-reports is worse than no instrument, because it invents a bug.
     */
    fun gesture(mode: String) {
        if (!on) return
        when (mode) {
            "days" -> panDays++
            "hours" -> panHours++
            "zoom" -> zooms++
            "tap" -> taps++
        }
        flush(System.nanoTime())
    }

    /** A week committed. */
    fun turned() {
        if (on) turns++
    }

    /** A settled pinch, with the rung it landed on, the numbers, not the gesture. */
    fun settled(hours: Int, columns: Int) {
        if (on) Log.d(TAG, "zoom settled: horizon=${hours}h columns=$columns")
    }

    private fun flush(now: Long) {
        if (lastFlush == 0L) lastFlush = now
        val elapsed = now - lastFlush
        if (elapsed < 1_000_000_000L) return

        val avgDrawUs = if (frames > 0) drawNanos / frames / 1_000 else 0
        val worstUs = worstDrawNanos / 1_000
        Log.d(
            TAG,
            "1s: frames=$frames draw_avg=${avgDrawUs}us draw_worst=${worstUs}us " +
                "blocks=$blocksDrawn culled=$blocksCulled measures=$measures " +
                "paints=$pagesPainted | pan_x=$panDays pan_y=$panHours zoom=$zooms tap=$taps " +
                "turns=$turns",
        )
        frames = 0
        blocksDrawn = 0
        blocksCulled = 0
        measures = 0
        drawNanos = 0
        worstDrawNanos = 0
        pagesPainted = 0
        panDays = 0
        panHours = 0
        zooms = 0
        taps = 0
        turns = 0
        lastFlush = now
    }
}
