// The periodic background mail sync (docs/background-sync.md). WorkManager runs this ~every
// 15 min while the app is backgrounded or killed: it builds a *headless* core (no standing
// IDLE/poll watches, one bounded pass, then quiesce), runs the core's `run_background_sync`,
// and raises a notification per newly-arrived inbound message. Best-effort by design, the OS
// may defer or drop a run under battery pressure, which is acceptable for this iteration.
package eu.allodia.mailcal

import android.content.Context
import android.util.Log
import androidx.work.CoroutineWorker
import androidx.work.WorkerParameters
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import uniffi.mailcal_bindings.BackgroundSyncOutcome
import uniffi.mailcal_bindings.MailcalApp
import uniffi.mailcal_bindings.deviceTimeZone

/// Whether a pass is running in this process, so two overlapping WorkManager requests cannot each
/// build a core over the same accounts. A plain flag rather than a lock: the second pass must be
/// *skipped*, not queued, by the time the first finishes, its window has moved on and its mail is
/// already fetched.
internal object PassInFlight {
    private val running = java.util.concurrent.atomic.AtomicBoolean(false)

    /// Takes the slot, or `false` if a pass already holds it.
    fun claim(): Boolean = running.compareAndSet(false, true)

    fun release() {
        running.set(false)
    }
}

class MailSyncWorker(context: Context, params: WorkerParameters) :
    CoroutineWorker(context, params) {

    override suspend fun doWork(): Result = withContext(Dispatchers.IO) {
        val context = applicationContext
        // Record the conditions BEFORE the foreground check, because a skipped pass is still a
        // *wake*, and the gap between wakes is exactly what tells us whether the OS is honouring
        // the 15-minute period or deferring us for hours. Logging this only on passes that do work
        // would hide the deferral we most need to see. See SyncPassDiagnostics.
        val now = System.currentTimeMillis()
        logPass(
            passConditionsLine(
                PassConditions(
                    minutesSinceLastWake = LastWake.minutesSince(context, now),
                    batteryExempt = BatteryOptimization.isExempt(context),
                    transport = activeTransport(context),
                    runAttempt = runAttemptCount,
                ),
            ),
        )
        LastWake.record(context, now)

        // Foreground: the live IMAP IDLE / poll runtime is already delivering mail into the open
        // app, so a background pass would only duplicate work and notify for mail the user can
        // already see. Let the foreground runtime own it.
        if (MailcalApplication.isForeground) {
            logPass("skipped: app is foregrounded; the live runtime owns delivery")
            return@withContext Result.success()
        }
        // One pass at a time in this process. WorkManager will happily run a one-time request
        // alongside the periodic one, they are different unique-work chains, and each pass builds
        // its own core, so two overlapping passes were two independent refreshers of the same
        // credential. Measured on a device: two cold cores 6 ms apart, both rotating the same
        // account's refresh token 168 ms apart, which on a ratcheting server is the replay that
        // revokes the grant.
        //
        // The core is now safe under that (its token state is shared per account across the
        // process, `CredentialOrigin`), so this is not the fix; it is the reason not to *need* the
        // fix. A second pass would duplicate every socket and every SQLite handle to fetch mail the
        // first is already fetching, inside an OS window that is about to close.
        if (!PassInFlight.claim()) {
            logPass("skipped: another background pass is already running in this process")
            return@withContext Result.success()
        }
        try {
            val live = MailcalApplication.liveCore?.get()
            if (live != null) {
                // Backgrounded but the process is still alive: reuse the live core rather than
                // opening a SECOND core over the same store (two SQLite/IMAP handles). A pass is
                // safe on a warm instance, a concurrent poll is absorbed as a Busy skip, and its
                // notify marks stay in one place, so no cross-core preferences race. Do NOT `use`/
                // close it: the foreground app owns its lifecycle.
                logPass("start: reusing the live (warm) core")
                postFrom(context, live.runBackgroundSync(BUDGET_SECONDS))
                return@withContext Result.success()
            }
            val configs = SecureStore.configs(context)
            if (configs.isEmpty()) {
                logPass("skipped: no stored accounts")
                return@withContext Result.success()
            }
            // Cold process (killed while backgrounded): build a headless core over the same store +
            // secure configs the Activity uses, sync once, notify, then drop it (the `use` closes
            // the FFI object, freeing the Rust runtime). No standing watches survive the run.
            logPass("start: building a headless (cold) core over ${configs.size} account(s)")
            MailcalApp.newBackgroundWorker(
                NoopObserver,
                CoreLogger,
                // The user's persisted DEBUG opt-in applies to this headless core too, a support
                // session's trace must not lose exactly the background passes it is chasing.
                DiagnosticsPrefs.bootLogLevel(context),
                configs,
                context.filesDir.absolutePath,
                deviceTimeZone(),
                DeviceFacts.of(context),
                // The same secure-store writer the Activity hands its core: this pass refreshes
                // tokens like any other, and the core is dropped when it ends. A rotation with
                // nowhere to go is lost, and against a server that detects a replayed refresh
                // token (Fastmail's ratchet) presenting the superseded one later revokes the whole
                // grant, which is exactly how a real JMAP account died.
                SecureStoreCredentialStore(context),
            ).use { core ->
                postFrom(context, core.runBackgroundSync(BUDGET_SECONDS))
            }
            Result.success()
        } catch (e: Exception) {
            logPass("failed: ${e.message}; the next periodic pass picks it up")
            FAILED_PASS_RESULT
        } finally {
            // Every exit path, including the early `return@withContext`es above, a slot never
            // released is a process that stops syncing in the background until it is restarted,
            // which is a worse bug than the one the guard prevents.
            PassInFlight.release()
        }
    }

    /// Raises new-mail notifications from a pass's outcome, honouring the user's toggle.
    private fun postFrom(context: Context, outcome: BackgroundSyncOutcome) {
        val newMessages = outcome.accounts.sumOf { it.newCount.toInt() }
        logPass(
            "complete: ${outcome.accounts.size} account(s) with new mail, " +
                "$newMessages new message(s)" + if (outcome.timedOut) " (budget timed out)" else "",
        )
        if (NotificationPrefs.enabled(context)) {
            MailNotifier.notifyNewMail(context, outcome)
        }
    }

    /// Records one background-pass lifecycle line to BOTH Logcat and the rotating file log (the
    /// same file the core's CoreLogger writes to). Tagged `[background-worker]` in the file so a
    /// background pass is unmistakable when reading the attachable log, its own start/complete
    /// markers bracket the core's interleaved `[mailcal_app::sync]` lines, and are distinct from
    /// the foreground `[android-ui]` records. See docs/background-sync.md + docs/logging.md.
    private fun logPass(message: String) {
        Log.i(TAG, "[background] $message")
        FileLog.append("INFO", "background-worker", message)
    }

    internal companion object {
        const val TAG = "Mailcal"

        // Well under WorkManager's ~10-min execution cap; a poll-once over a few accounts is
        // quick, and the core clamps this into a sane band regardless.
        const val BUDGET_SECONDS = 120u

        /// What a *failed* pass reports to WorkManager. `success()`, deliberately, and this is
        /// the one thing in this file not to "fix".
        ///
        /// `retry()` hands this periodic work over to WorkManager's exponential backoff: 30s, 1m,
        /// 2m, 4m … doubling, capped at five hours, and `runAttemptCount` only resets on a pass
        /// that succeeds. From the sixth consecutive failure the backoff is already longer than
        /// the 15-minute period it replaced, and it stays there until something succeeds. A phone
        /// that moves between 5G, Wi-Fi and no coverage fails passes often enough to get there, so
        /// mail delivery silently decays from every 15 minutes to hours apart, with no error
        /// anywhere, because backing off is what retry() is *supposed* to do. **The period is the
        /// retry**: the next tick is 15 minutes out, sooner than any backoff worth having.
        ///
        /// `failure()` is not the alternative either, for periodic work it is terminal and
        /// cancels every future run, so background mail would stop for good.
        val FAILED_PASS_RESULT: Result = Result.success()
    }
}
