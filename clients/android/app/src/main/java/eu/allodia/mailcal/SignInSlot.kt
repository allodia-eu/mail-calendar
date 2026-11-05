// One browser sign-in whose tab runs in a task of its own (IMAP, JMAP): the spinner the screen
// draws, the pending handle the redirect completes, what the attempt is for, and which attempt is
// the current one. A press supersedes the sign-in before it (docs/provider-oauth.md rule 13), and
// leaving the card retires it, so nothing still running can open a browser or add an account for
// it (docs/onboarding.md, "Nobody may be stranded on it"). Compose-free apart from the spinner's
// state, so the JVM suite reaches it.
package eu.allodia.mailcal

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue

/** A redirect that found a sign-in armed: the attempt it completes, its handle and its [flow]. */
internal data class Landed<F>(val attempt: Long, val handle: String, val flow: F)

internal class SignInSlot<F : Any> {
    /** Whether the screen draws its sign-in busy. */
    var signingIn by mutableStateOf(false)
        private set

    /** The handle the redirect completes, held while the browser has the sign-in. */
    var pending: String? = null
        private set

    /** What the current attempt is for, or null when none is running or armed. */
    var flow: F? = null
        private set

    private var attempt = 0L

    /** A press: whatever sign-in was running or pending is superseded. Names the new attempt. */
    fun begin(flow: F): Long {
        attempt += 1
        pending = null
        this.flow = flow
        signingIn = true
        return attempt
    }

    /**
     * The browser is about to open for [attempt] with [handle]. False when the attempt was retired
     * or superseded meanwhile, and then no browser opens.
     */
    fun opening(attempt: Long, handle: String): Boolean {
        if (attempt != this.attempt || flow == null) return false
        pending = handle
        return true
    }

    /**
     * The app is in front again. With the handle still pending the redirect has not landed, so the
     * person is looking at the screen and the spinner goes. The handle stays armed: the tab runs in
     * a task of its own, so it may still be open, and finishing there must still complete. Before
     * the browser opens nothing is pending, and the spinner stays for the start still running. Says
     * whether it cleared the spinner.
     */
    fun resumed(): Boolean {
        if (!signingIn || pending == null) return false
        signingIn = false
        return true
    }

    /**
     * The redirect landed: what it completes, which leaves the slot, or null when nothing was armed
     * for it. The screen is busy again while the code is exchanged.
     */
    fun redirected(): Landed<F>? {
        val handle = pending ?: return null
        val flow = flow ?: return null
        pending = null
        signingIn = true
        return Landed(attempt, handle, flow)
    }

    /**
     * [attempt] answered, by failing to start or with its exchange. False when it was retired or
     * superseded meanwhile, and then its answer is dropped.
     */
    fun settled(attempt: Long): Boolean {
        if (attempt != this.attempt || flow == null) return false
        flow = null
        signingIn = false
        return true
    }

    /** The person left the screen the sign-in was started from. */
    fun retire() {
        attempt += 1
        pending = null
        flow = null
        signingIn = false
    }
}
