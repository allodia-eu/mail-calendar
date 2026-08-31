// Shared plumbing for the headless background-sync core the WorkManager worker builds
// (docs/background-sync.md): a no-op Observer (a background pass reads `run_background_sync`'s
// return value directly, it renders no UI) and a Logger that routes the core's records to
// Logcat + the rotating file log, exactly as the foreground Activity does, so background
// diagnostics land in the same attachable file (docs/logging.md).
package eu.allodia.mailcal

import android.util.Log
import uniffi.mailcal_bindings.LogLevel
import uniffi.mailcal_bindings.Logger
import uniffi.mailcal_bindings.Observer
import uniffi.mailcal_bindings.Surface

private const val TAG = "Mailcal"

/// A background pass drives the core headlessly and reads its result directly, so surface
/// signals need no handling, the core still emits them during the refresh, harmlessly dropped.
object NoopObserver : Observer {
    override fun surfaceChanged(surface: Surface) {}
}

/// Forwards the core's log records to Logcat and the rotating file log, the single log sink for
/// both the foreground Activity (via `newAccounts`) and the background-sync worker, so every layer
/// lands in one attachable stream (docs/logging.md). The core gates by level before crossing the
/// FFI and never logs sensitive content.
object CoreLogger : Logger {
    override fun log(level: LogLevel, target: String, message: String) {
        val line = "[$target] $message"
        when (level) {
            LogLevel.ERROR -> Log.e(TAG, line)
            LogLevel.WARN -> Log.w(TAG, line)
            LogLevel.INFO -> Log.i(TAG, line)
            LogLevel.DEBUG -> Log.d(TAG, line)
            LogLevel.TRACE -> Log.v(TAG, line)
        }
        FileLog.append(level.name, target, message)
    }
}
