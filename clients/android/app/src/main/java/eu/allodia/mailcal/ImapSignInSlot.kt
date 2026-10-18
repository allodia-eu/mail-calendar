// The one IMAP browser sign-in this activity holds: the spinner the setup card draws, the pending
// handle the redirect completes, and which attempt is the current one. A press supersedes the
// sign-in before it (docs/provider-oauth.md rule 13), and leaving the card retires it, so nothing
// still running can open a browser or add an account for it (docs/onboarding.md, "Nobody may be
// stranded on it"). Compose-free apart from the spinner's state, so the JVM suite reaches it.
package eu.allodia.mailcal

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue

internal class ImapSignInSlot {
    /** Whether the card draws its sign-in button busy. */
    var signingIn by mutableStateOf(false)
        private set

    /** The handle the redirect completes, held while the browser has the sign-in. */
    var pending: String? = null
        private set

    private var attempt = 0L

    /** A press of the button: whatever sign-in was pending is superseded. Names the new attempt. */
    fun begin(): Long {
        attempt += 1
        pending = null
        signingIn = true
        return attempt
    }

    /**
     * The browser is about to open for [attempt] with [handle]. False when the attempt was retired
     * or superseded meanwhile, and then no browser opens.
     */
    fun opening(attempt: Long, handle: String): Boolean {
        if (attempt != this.attempt) return false
        pending = handle
        return true
    }

    /**
     * The app is in front again. With the handle still pending the redirect has not landed, so the
     * person is looking at the card and the spinner goes. The handle stays armed: the tab runs in a
     * task of its own, so it may still be open, and finishing there must still complete. Before the
     * browser opens nothing is pending, and the spinner stays for the start still running. Says
     * whether it cleared the spinner.
     */
    fun resumed(): Boolean {
        if (!signingIn || pending == null) return false
        signingIn = false
        return true
    }

    /**
     * The redirect landed: the attempt it completes and the handle, which leaves the slot, or null
     * when nothing was armed for it. The card is busy again while the code is exchanged.
     */
    fun redirected(): Pair<Long, String>? {
        val handle = pending ?: return null
        pending = null
        signingIn = true
        return attempt to handle
    }

    /**
     * [attempt] answered, by failing to start or with its exchange. False when it was retired or
     * superseded meanwhile, and then its answer is dropped.
     */
    fun settled(attempt: Long): Boolean {
        if (attempt != this.attempt) return false
        signingIn = false
        return true
    }

    /** The person left the card the sign-in was started from. */
    fun retire() {
        attempt += 1
        pending = null
        signingIn = false
    }
}
