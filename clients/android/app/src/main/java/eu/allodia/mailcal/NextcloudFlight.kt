// The browser half of a Nextcloud sign-in, which every surface offering it shares
// (docs/nextcloud-sign-in.md rules 5 and 6). There is no redirect back: the core starts the
// sign-in, a Custom Tab opens on the server's own login page, and a worker asks the core whether
// the person has finished, every poll interval and at once when the app is in front again.
// Compose-free, so the JVM suite drives the wait.
package eu.allodia.mailcal

import java.util.concurrent.LinkedBlockingQueue
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicReference
import uniffi.mailcal_bindings.NextcloudSignInPoll

/** How a flight ended. A grant carries the app password: held in memory only, never logged. */
internal sealed interface NextcloudEnd {
    class Granted(val grant: String) : NextcloudEnd {
        override fun toString() = "Granted(..)"
    }

    /** Twenty minutes passed and the server let the sign-in go. */
    data object Expired : NextcloudEnd

    /** The sign-in did not start, the browser did not open, or the grant could not be read. */
    data class Failed(val error: Throwable?) : NextcloudEnd

    /** The attempt was retired or superseded. */
    data object Cancelled : NextcloudEnd
}

internal class NextcloudFlight {
    private val wakes = LinkedBlockingQueue<Unit>()
    private val stopped = AtomicReference<NextcloudEnd?>(null)

    /** Whether the flight was stopped, so nothing it finds may act for anyone. */
    val isStopped: Boolean get() = stopped.get() != null

    /** The app is in front again: ask now rather than at the next interval. */
    fun wake() {
        wakes.offer(Unit)
    }

    /** Ends the flight with [end] at its next ask; the first stop is the one it reports. */
    fun stop(end: NextcloudEnd = NextcloudEnd.Cancelled) {
        stopped.compareAndSet(null, end)
        wakes.offer(Unit)
    }

    /**
     * Asks [poll] until the sign-in ends or the flight is stopped, waiting [intervalMs] between
     * asks, or less after a [wake]. BLOCKS, so it runs on the flight's worker.
     */
    fun await(intervalMs: Long, poll: () -> NextcloudSignInPoll): NextcloudEnd {
        while (true) {
            stopped.get()?.let { return it }
            val answer = try {
                poll()
            } catch (e: Exception) {
                return NextcloudEnd.Failed(e)
            }
            when (answer) {
                // Returned even when stopped meanwhile, so the caller withdraws it: the server
                // hands it out once, and nobody else will.
                is NextcloudSignInPoll.Granted -> return NextcloudEnd.Granted(answer.grant)
                NextcloudSignInPoll.Expired -> return NextcloudEnd.Expired
                NextcloudSignInPoll.Waiting -> wakes.poll(intervalMs, TimeUnit.MILLISECONDS)
            }
        }
    }
}
