// What a FAILED background pass reports to WorkManager (docs/background-sync.md).
//
// This pins a one-line policy that is easy to "correct" back into a bug. A failed pass returns
// `Result.success()`. Returning `retry()` instead hands the periodic work to WorkManager's
// exponential backoff, 30s, 1m, 2m, 4m … doubling to a five-hour cap, with `runAttemptCount`
// resetting only on a pass that succeeds. From the sixth consecutive failure the backoff is already
// longer than the 15-minute period it replaced, and it stays there until something succeeds. A phone
// moving between 5G, Wi-Fi and no coverage fails often enough to get there, and background mail then
// decays from every 15 minutes to hours apart with no error raised anywhere, because backing off is
// precisely what retry() is meant to do. The period IS the retry.
//
// `failure()` is not the alternative: for periodic work it is terminal and cancels every future run.
//
// The worker's own `doWork()` is not reachable from the JVM suite, its failing path runs through
// `MailcalApp.newBackgroundWorker`, which loads the cdylib that this suite deliberately never loads
// (AGENTS.md). So the policy is asserted here, and the end-to-end behaviour is verified on a device
// by watching the job's next run stay ~15 min out after a failed pass rather than backing off.
package eu.allodia.mailcal

import androidx.work.ListenableWorker.Result
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotEquals
import org.junit.Test

class MailSyncRetryPolicyTest {

    @Test
    fun `a failed pass reports success, so the 15-minute period is not replaced by a backoff`() {
        assertEquals(Result.success(), MailSyncWorker.FAILED_PASS_RESULT)
    }

    @Test
    fun `a failed pass never asks WorkManager to retry`() {
        assertNotEquals(
            "retry() escalates a periodic worker's interval past its own period and only resets " +
                "on a success, a phone that keeps losing coverage would decay to hours between " +
                "passes",
            Result.retry(),
            MailSyncWorker.FAILED_PASS_RESULT,
        )
    }

    @Test
    fun `a failed pass never reports failure, which would cancel the periodic work for good`() {
        assertNotEquals(
            "failure() is terminal for periodic work: background mail would stop permanently",
            Result.failure(),
            MailSyncWorker.FAILED_PASS_RESULT,
        )
    }
}
