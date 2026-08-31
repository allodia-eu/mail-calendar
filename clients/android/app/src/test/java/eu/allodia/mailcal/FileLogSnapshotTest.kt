// The Diagnostics screen's read side of FileLog: the size/backup math its status rows
// show and the viewer's read of the current file.
//
// Plain JUnit over a temp dir, no Robolectric. `snapshotOf`/`textOf` are pure file math, factored
// out of the FileLog singleton precisely so the size totals, the backup counting, and the
// missing-file behaviour are pinned without Android (`Build.*`) or the singleton's process-wide
// `file` state in the way.
package eu.allodia.mailcal

import java.io.File
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder

class FileLogSnapshotTest {
    @get:Rule
    val tmp = TemporaryFolder()

    private fun log(): File = File(tmp.root, "app.log")

    @Test
    fun the_fault_handler_and_the_share_sheet_name_one_file() {
        // The native-fault handler cannot go through `append`, it runs in a signal handler, so it
        // opens `FileLog.path()` itself. If that ever drifts from the file the sink writes and
        // Diagnostics shares, the record still gets written and nobody ever sees it: no error, no
        // exception, just a crash the support log says nothing about.
        val handed = FileLog.fileIn(tmp.root.absolutePath)

        assertEquals(File(File(tmp.root, "logs"), "app.log").absolutePath, handed.absolutePath)
        assertEquals(handed.absolutePath, FileLog.snapshotOf(handed).path)
    }

    @Test
    fun the_total_spans_the_current_log_and_every_backup() {
        log().writeText("a".repeat(10))
        File(tmp.root, "app.log.1").writeText("b".repeat(20))
        File(tmp.root, "app.log.3").writeText("c".repeat(40))

        assertEquals(70L, FileLog.snapshotOf(log()).totalBytes)
    }

    @Test
    fun backups_are_counted_by_presence_never_assumed_contiguous() {
        // Rotation produces .1..3 in order, but the counter must count what is on disk, not what
        // rotation would have produced, a user (or a bug) can leave holes.
        log().writeText("x")
        File(tmp.root, "app.log.1").writeText("x")
        File(tmp.root, "app.log.3").writeText("x")

        assertEquals(2, FileLog.snapshotOf(log()).backupCount)
    }

    @Test
    fun a_missing_log_reports_zero_rather_than_failing() {
        val snap = FileLog.snapshotOf(log())

        assertEquals(0L, snap.totalBytes)
        assertEquals(0, snap.backupCount)
        assertEquals(log().absolutePath, snap.path)
    }

    @Test
    fun only_the_numbered_backups_count_toward_the_footprint() {
        // A stray sibling must not inflate the reported size: rotation keeps at most .1..3.
        log().writeText("x")
        File(tmp.root, "app.log.4").writeText("not a backup")
        File(tmp.root, "unrelated.txt").writeText("unrelated")

        val snap = FileLog.snapshotOf(log())

        assertEquals(1L, snap.totalBytes)
        assertEquals(0, snap.backupCount)
    }

    @Test
    fun the_viewer_reads_the_current_file_only_newest_last() {
        // Appends are chronological, so the file's own order IS "newest last", and a rotated
        // backup never leaks into the viewer.
        log().writeText("older line\nnewest line\n")
        File(tmp.root, "app.log.1").writeText("rotated away\n")

        assertEquals("older line\nnewest line\n", FileLog.textOf(log()))
    }

    @Test
    fun reading_a_missing_file_is_the_empty_state_not_an_error() {
        assertEquals("", FileLog.textOf(log()))
    }

    @Test
    fun the_session_marker_names_the_build_the_log_came_from() {
        // docs/logging.md → "the shared bar": a log handed to support is only actionable if it says
        // which build wrote it, and /VERSION holds the last *released* version, so the versionName
        // alone cannot tell a dev build from the shipped one, and the derived versionCode must be
        // there too. Asserted on the shape, not on today's numbers, so a release never breaks it.
        val marker = FileLog.sessionMarker("Pixel 8, Android 15")

        assertTrue(
            "marker must carry <version> build <code>, got: $marker",
            marker.matches(Regex("""--- session start \(\d+\.\d+\.\d+ build \d+, Pixel 8, Android 15\) ---""")),
        )
    }
}
