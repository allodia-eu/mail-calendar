// The browser sign-in a separate server's form holds (docs/mail-oauth.md rule 13): where it is,
// the pending handle the redirect completes, and which attempt is the current one. A press
// supersedes the sign-in before it (docs/provider-oauth.md rule 13), and leaving the form retires
// it, so nothing still running can open a browser or add an account for it (docs/onboarding.md,
// "Nobody may be stranded on it"). The setup IMAP sign-in's slot has the same shape. Compose-free
// apart from the step's state, so the JVM suite reaches it.
package eu.allodia.mailcal

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue

/** Where a separate server's browser sign-in is. */
internal enum class SeparateSignInStep {
    /** The core asks the server who signs it in and registers with that issuer; blocks. */
    STARTING,

    /** The browser has it, and the form waits for the redirect. */
    BROWSER,

    /** The redirect arrived; the exchange and the move to a new account block. */
    COMPLETING,
}

internal class SeparateServerSignInSlot {
    /** Where the sign-in is, or null when none runs or the person came back from the browser. */
    var step by mutableStateOf<SeparateSignInStep?>(null)
        private set

    /** The handle the redirect completes, held while the browser has the sign-in. */
    var pending: String? = null
        private set

    private var attempt = 0L

    /** A press of the button: whatever sign-in was pending is superseded. Names the new attempt. */
    fun begin(): Long {
        attempt += 1
        pending = null
        step = SeparateSignInStep.STARTING
        return attempt
    }

    /**
     * The browser is about to open for [attempt] with [handle]. False when the attempt was retired
     * or superseded meanwhile, and then no browser opens.
     */
    fun opening(attempt: Long, handle: String): Boolean {
        if (attempt != this.attempt) return false
        pending = handle
        step = SeparateSignInStep.BROWSER
        return true
    }

    /**
     * The app is in front again. With the handle still pending the redirect has not landed, so the
     * person is looking at the form and the spinner goes. The handle stays armed: the tab runs in a
     * task of its own, so it may still be open, and finishing there must still complete. Says
     * whether it cleared the spinner.
     */
    fun resumed(): Boolean {
        if (step != SeparateSignInStep.BROWSER || pending == null) return false
        step = null
        return true
    }

    /**
     * The redirect landed: the attempt it completes and the handle, which leaves the slot, or null
     * when nothing was armed for it.
     */
    fun redirected(): Pair<Long, String>? {
        val handle = pending ?: return null
        pending = null
        step = SeparateSignInStep.COMPLETING
        return attempt to handle
    }

    /**
     * [attempt] answered, by failing to start or with its completion. False when it was retired or
     * superseded meanwhile, and then its answer is dropped.
     */
    fun settled(attempt: Long): Boolean {
        if (attempt != this.attempt) return false
        step = null
        return true
    }

    /** The form was left, or another way in was taken: nothing still running may act for it. */
    fun retire() {
        attempt += 1
        pending = null
        step = null
    }
}
