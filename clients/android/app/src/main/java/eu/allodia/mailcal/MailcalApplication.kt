// The Application: owns process-wide setup that must exist whether the process was started by
// the user (the Activity) or by WorkManager (a cold background sync). It opens the rotating file
// log, schedules the periodic background mail sync, and tracks whether any Activity is in the
// foreground (so the sync worker can defer to the live runtime). See docs/background-sync.md.
package eu.allodia.mailcal

import android.app.Activity
import android.app.Application
import android.content.Context
import android.os.Bundle
import androidx.work.Constraints
import androidx.work.ExistingPeriodicWorkPolicy
import androidx.work.NetworkType
import androidx.work.PeriodicWorkRequestBuilder
import androidx.work.WorkManager
import java.lang.ref.WeakReference
import java.util.concurrent.TimeUnit
import uniffi.mailcal_bindings.MailcalApp

class MailcalApplication : Application() {
    override fun onCreate() {
        super.onCreate()
        // Runs on every process start (Activity launch *and* a cold WorkManager wake), so boot
        // diagnostics are captured from the first line in both.
        FileLog.init(filesDir.absolutePath)
        // Straight after the sink exists, so a crash during the rest of startup still writes.
        CrashLog.watchForCrashes()
        FileLog.path()?.let(CrashLog::watchForNativeFaults)
        registerActivityLifecycleCallbacks(ForegroundTracker)
        MailSyncScheduler.schedule(this)
    }

    companion object {
        /// Whether an Activity is currently started (foreground). Read by the sync worker to
        /// defer to the live runtime while the app is open. Volatile, read off a worker thread.
        @Volatile
        var isForeground: Boolean = false
            internal set

        /// A **weak** handle to the foreground live core. The sync worker reuses it while the
        /// process is still alive (backgrounded but not yet killed) instead of opening a SECOND
        /// core over the same store (two SQLite/IMAP handles). Weak so it clears once the Activity's
        /// core is gone, a cold process (killed while backgrounded) then builds a headless core.
        /// Set by `MainActivity.connect`. See docs/background-sync.md.
        @Volatile
        var liveCore: WeakReference<MailcalApp>? = null
            internal set
    }
}

/// Tracks foreground state by counting started Activities, so a single flag reflects whether the
/// app is currently on screen (started, not merely created).
private object ForegroundTracker : Application.ActivityLifecycleCallbacks {
    private var started = 0

    override fun onActivityStarted(activity: Activity) {
        started++
        MailcalApplication.isForeground = true
    }

    override fun onActivityStopped(activity: Activity) {
        started--
        if (started <= 0) MailcalApplication.isForeground = false
    }

    override fun onActivityCreated(activity: Activity, savedInstanceState: Bundle?) {}
    override fun onActivityResumed(activity: Activity) {}
    override fun onActivityPaused(activity: Activity) {}
    override fun onActivitySaveInstanceState(activity: Activity, outState: Bundle) {}
    override fun onActivityDestroyed(activity: Activity) {}
}

/// Enqueues the ~15-min periodic background sync. `KEEP` so re-enqueuing on each launch never
/// resets an already-scheduled chain; a `CONNECTED` constraint keeps a run from firing offline.
/// WorkManager persists and reschedules the work across process death and reboots on its own.
object MailSyncScheduler {
    private const val WORK_NAME = "mail-sync"

    fun schedule(context: Context) {
        val constraints = Constraints.Builder()
            .setRequiredNetworkType(NetworkType.CONNECTED)
            .build()
        val request = PeriodicWorkRequestBuilder<MailSyncWorker>(15, TimeUnit.MINUTES)
            .setConstraints(constraints)
            .build()
        WorkManager.getInstance(context).enqueueUniquePeriodicWork(
            WORK_NAME,
            ExistingPeriodicWorkPolicy.KEEP,
            request,
        )
    }

    /// Cancels the periodic sync. Used only by a showcase (screenshot) run: the worker builds its
    /// own headless core over the *stored* accounts, so left running it would sync the developer's
    /// real mail and could raise a real new-mail notification, sender and subject and all, over
    /// the screenshot being taken. The next normal launch re-enqueues it.
    fun cancel(context: Context) {
        WorkManager.getInstance(context).cancelUniqueWork(WORK_NAME)
    }
}
