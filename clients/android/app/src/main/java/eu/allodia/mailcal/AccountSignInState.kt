// An IMAP or JMAP account's sign-in being switched or renewed (docs/accounts.md rule 17): what
// each account's server said it takes, and the one flow running. Held by the activity, because a
// browser sign-in leaves the app and comes back through onNewIntent; Compose-free in its decisions,
// so the JVM suite drives them.
package eu.allodia.mailcal

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import uniffi.mailcal_bindings.AccountSignInOptions

/** Where a running sign-in is. */
internal enum class SignInStep {
    /** The core is building the authorization URL, which may discover and register. */
    STARTING,

    /** The browser is open, waiting for the redirect. */
    BROWSER,

    /** The redirect arrived; the exchange and the connect run. */
    COMPLETING,

    /** A password is being tried. */
    PASSWORD,
}

/** A sign-in running for [accountId]. [switching] is a change of method, not a sign-in again. */
internal data class SignInRun(val accountId: String, val step: SignInStep, val switching: Boolean)

/** The browser half of a running sign-in, which the next IMAP or JMAP redirect completes. */
internal data class PendingAccountSignIn(
    val accountId: String,
    val handle: String,
    val switching: Boolean,
)

internal class AccountSignInState {
    /** What each account's server answered, by account id. */
    var options by mutableStateOf<Map<String, AccountSignInOptions>>(emptyMap())
        private set

    private val asking = mutableSetOf<String>()

    /** The flow running, one at a time: a second browser would claim the first one's redirect. */
    var running by mutableStateOf<SignInRun?>(null)
        private set

    /** The sign-in the next IMAP or JMAP redirect completes, while one waits. */
    var pending: PendingAccountSignIn? = null
        private set

    /** Counts switches to a password, so the field empties after each. */
    var passwordsSaved by mutableIntStateOf(0)
        private set

    /** Whether to ask [accountId]'s server now: false while a question is already out. */
    fun startAsking(accountId: String): Boolean = asking.add(accountId)

    /** The server answered for [accountId]. */
    fun answered(accountId: String, answer: AccountSignInOptions) {
        asking.remove(accountId)
        options = options + (accountId to answer)
    }

    /** Whether nothing runs, so a flow may start. */
    val idle: Boolean get() = running == null

    /** Starts a browser sign-in for [accountId]; false when another flow runs. */
    fun begin(accountId: String, switching: Boolean): Boolean {
        if (running != null) return false
        running = SignInRun(accountId, SignInStep.STARTING, switching)
        return true
    }

    /** Starts trying a password for [accountId]; false when another flow runs. */
    fun beginPassword(accountId: String): Boolean {
        if (running != null) return false
        running = SignInRun(accountId, SignInStep.PASSWORD, switching = true)
        return true
    }

    /** The browser opens for the running sign-in, which [handle] completes. */
    fun opened(handle: String) {
        val run = running ?: return
        pending = PendingAccountSignIn(run.accountId, handle, run.switching)
        running = run.copy(step = SignInStep.BROWSER)
    }

    /** A redirect arrived: the sign-in it completes, now spent, or null when none waits. */
    fun claim(): PendingAccountSignIn? {
        val claimed = pending ?: return null
        pending = null
        running = SignInRun(claimed.accountId, SignInStep.COMPLETING, claimed.switching)
        return claimed
    }

    /**
     * The app came back to the front. The browser runs in its own task, so it may still be open
     * behind a password manager: the page stops saying it waits, and the handle stays armed so a
     * redirect that still arrives completes it.
     */
    fun resumed() {
        if (running?.step == SignInStep.BROWSER) running = null
    }

    /** A sign-in from the setup form or the banner claims the redirect this one waited for. */
    fun yieldRedirect() {
        if (pending == null) return
        pending = null
        if (running?.step == SignInStep.BROWSER) running = null
    }

    /**
     * The flow ended. A switch that took effect asks the server again, since what it takes is
     * asked of the way the account now signs in.
     */
    fun finished(switched: Boolean = false) {
        val run = running
        running = null
        if (switched && run != null) {
            options = options - run.accountId
            if (run.step == SignInStep.PASSWORD) passwordsSaved += 1
        }
    }
}
