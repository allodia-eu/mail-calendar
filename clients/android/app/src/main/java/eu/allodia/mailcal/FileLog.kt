package eu.allodia.mailcal

import android.os.Build
import java.io.File
import java.time.LocalDateTime
import java.time.format.DateTimeFormatter

/// A point-in-time view of the log's on-disk footprint, the current file plus its numbered
/// backups, for the Diagnostics screen's status rows and share/copy actions.
internal data class LogSnapshot(val path: String, val totalBytes: Long, val backupCount: Int)

// A size-rotating file log for field debugging, the Android counterpart of the Windows
// client's file Log (`Services/Log.cs`) and the macOS `FileLog`. The core's diagnostics
// (routed through the FFI `Logger`) also go to Logcat, but a file survives the process and is
// trivial to attach to a support report. Path: <filesDir>/logs/app.log.
//
// Rotation is size-based, at 1 MB, app.log -> app.log.1 -> ... -> app.log.3 (oldest dropped),
// so the logs cap at ~4 MB total and never grow unbounded. See docs/logging.md.
//
// Privacy-safe: the core logs counts, ids, and high-level events, never mail/event content,
// addresses, or credentials. Best-effort: logging never throws.
object FileLog {
    private const val MAX_BYTES = 1L shl 20 // 1 MB per file
    private const val BACKUPS = 3           // app.log + .1..3 => ~4 MB cap

    private val lock = Any()
    // DateTimeFormatter is immutable/thread-safe (unlike SimpleDateFormat); java.time is
    // available unconditionally at minSdk 31.
    private val formatter = DateTimeFormatter.ofPattern("yyyy-MM-dd HH:mm:ss.SSS")

    @Volatile
    private var file: File? = null

    // Points the log at <dataDir>/logs/app.log and stamps a session-start line. Best-effort:
    // a log that can't open simply stays silent; it must not break startup.
    fun init(dataDir: String) {
        synchronized(lock) {
            file = try {
                fileIn(dataDir).also { it.parentFile?.mkdirs() }
            } catch (_: Throwable) {
                null
            }
        }
        append("INFO", "filelog", sessionMarker("${Build.MODEL}, Android ${Build.VERSION.RELEASE}"))
    }

    // The session-start line, with the device string passed in so the rule "it names the build" is
    // a test that can fail without needing an Android runtime (FileLogSnapshotTest).
    //
    // The app version and versionCode lead it: `/VERSION` holds the last *released* version, so a
    // dev build and a shipped one report the same versionName and only the derived versionCode
    // tells them apart (docs/versioning.md). Without this a log attached to a support request
    // cannot be pinned to a build at all (docs/logging.md → "the shared bar").
    internal fun sessionMarker(device: String): String =
        "--- session start (${BuildConfig.VERSION_NAME} build ${BuildConfig.VERSION_CODE}, " +
            "$device) ---"

    // Appends one line: `<timestamp> <LEVEL> [target] message`. Called from Rust runtime
    // worker threads, so the rotation check and write are serialized under `lock`.
    fun append(level: String, target: String, message: String) {
        val f = file ?: return
        val line = "${LocalDateTime.now().format(formatter)} $level [$target] $message\n"
        synchronized(lock) {
            try {
                rotate(f)
                f.appendText(line)
            } catch (_: Throwable) {
                // Logging is best-effort; a transient IO failure is swallowed.
            }
        }
    }

    // ---- The Diagnostics screen's read side ------------------------------------------------
    //
    // Same lock as `append`, so a size total or a viewer read never tears against a rotation
    // shuffling the files mid-walk; same best-effort discipline as the write side, an IO failure
    // returns null rather than throwing into the UI. The pure file math lives in `snapshotOf` /
    // `textOf` below so plain JUnit can pin it over a temp dir (no `Build.*` in the way).

    /// The log file inside [dataDir], `<dataDir>/logs/app.log`.
    ///
    /// Pure path math, factored out of `init` so a plain JUnit test can pin that the path handed
    /// to the native-fault handler is the file `append` writes and Diagnostics shares. A handler
    /// pointed anywhere else fails silently: it writes a record into a file nobody will read.
    internal fun fileIn(dataDir: String): File = File(File(dataDir, "logs"), "app.log")

    /// The current file's absolute path, or null before `init`.
    ///
    /// Handed to the core's native-fault handler, which cannot go through `append`: it runs in a
    /// signal handler, where taking `lock` or calling into the JVM is not permitted, so it opens
    /// this path itself (docs/logging.md → "A crash says so on the way out").
    internal fun path(): String? = file?.absolutePath

    /// The log's current footprint, or null before `init` (or on an IO failure).
    internal fun snapshot(): LogSnapshot? {
        val f = file ?: return null
        synchronized(lock) {
            return try {
                snapshotOf(f)
            } catch (_: Throwable) {
                null
            }
        }
    }

    /// The CURRENT app.log's text, chronological, so "newest last" is the file's own order.
    /// Backups are deliberately excluded: the viewer shows the live file; a support request
    /// attaches it via share. Empty string when nothing has been written yet; null before
    /// `init` (or on an IO failure).
    internal fun readCurrent(): String? {
        val f = file ?: return null
        synchronized(lock) {
            return try {
                textOf(f)
            } catch (_: Throwable) {
                null
            }
        }
    }

    /// Pure math over [logFile] and its `.1..BACKUPS` siblings: total bytes across whichever of
    /// them exist, and how many backups are present. Counts by presence, never assuming the set
    /// is contiguous, and ignores anything rotation would not have produced.
    internal fun snapshotOf(logFile: File): LogSnapshot {
        var total = if (logFile.exists()) logFile.length() else 0L
        var backups = 0
        for (i in 1..BACKUPS) {
            val backup = File("${logFile.path}.$i")
            if (backup.exists()) {
                backups += 1
                total += backup.length()
            }
        }
        return LogSnapshot(logFile.absolutePath, total, backups)
    }

    /// [logFile]'s text, or the empty string when it does not exist (a fresh install's viewer
    /// shows the empty state, not an error).
    internal fun textOf(logFile: File): String =
        if (logFile.exists()) logFile.readText() else ""

    // app.log.(BACKUPS-1) -> app.log.BACKUPS, ..., app.log -> app.log.1, dropping the oldest.
    // Each destination is vacated before its move. Mirrors the Windows client's `Log.Rotate`.
    private fun rotate(path: File) {
        if (!path.exists() || path.length() < MAX_BYTES) {
            return
        }
        File("${path.path}.$BACKUPS").delete()
        for (i in BACKUPS - 1 downTo 1) {
            val src = File("${path.path}.$i")
            if (src.exists()) {
                src.renameTo(File("${path.path}.${i + 1}"))
            }
        }
        path.renameTo(File("${path.path}.1"))
    }
}
