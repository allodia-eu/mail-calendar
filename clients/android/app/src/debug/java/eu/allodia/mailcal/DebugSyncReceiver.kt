// DEBUG-only (src/debug, never in a release build): a broadcast trigger that enqueues a
// one-time MailSyncWorker, so a live test can run the background sync on demand instead of
// waiting for the ~15-min periodic window (which `cmd jobscheduler run` can't force before its
// schedule). Trigger:
//   adb shell am broadcast -a <applicationId>.DEBUG_RUN_SYNC -p <applicationId>
// The id depends on how the build was branded (docs/branding.md); `scripts/dev/boot.sh android`
// and the debug-app skill fill it in.
package eu.allodia.mailcal

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.util.Log
import androidx.work.OneTimeWorkRequestBuilder
import androidx.work.WorkManager

class DebugSyncReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        Log.i("Mailcal", "[background] DEBUG_RUN_SYNC: enqueuing one-time MailSyncWorker")
        // Also record it in the attachable file log, so a manually triggered run is traceable
        // there alongside the worker's own pass markers (docs/logging.md).
        FileLog.append("INFO", "background-worker", "DEBUG_RUN_SYNC: enqueuing one-time worker")
        WorkManager.getInstance(context)
            .enqueue(OneTimeWorkRequestBuilder<MailSyncWorker>().build())
    }
}
