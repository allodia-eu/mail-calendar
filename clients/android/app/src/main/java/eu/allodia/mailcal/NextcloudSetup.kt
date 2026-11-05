// What setup knows about a sign-in with Nextcloud (docs/nextcloud-sign-in.md rules 2 and 6): the
// servers the manual form asked about, the attempt in the air, and a grant no account was made of
// yet, kept so the next press tries it again without the browser. Compose-free in its decisions,
// so the JVM suite drives them.
package eu.allodia.mailcal

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import uniffi.mailcal_bindings.AccountCapability

/** What the setup cards draw of it, and what their buttons send. */
internal data class NextcloudSetupUi(
    /** The sign-in button's spinner: the core starts the sign-in, or the browser holds it. */
    val signingIn: Boolean = false,
    /** Whether the server at a typed URL answered that it is Nextcloud. */
    val offers: (String) -> Boolean = { false },
    /** Asks whether the server at a typed URL is Nextcloud; once per URL. */
    val onAsk: (String) -> Unit = {},
    /** Signs in at the server, for the uses chosen. */
    val onSignIn: (server: String, uses: List<AccountCapability>) -> Unit = { _, _ -> },
)

internal class SetupNextcloud {
    var signingIn by mutableStateOf(false)
        private set

    /** The servers the manual form asked about, and whether each answered as Nextcloud. */
    private var answers by mutableStateOf<Map<String, Boolean>>(emptyMap())

    private val asked = mutableSetOf<String>()
    private var attempt = 0L
    private var flight: NextcloudFlight? = null
    private var held: Pair<String, String>? = null

    fun offers(server: String): Boolean = answers[server.trim()] == true

    /** Whether to ask about [server] now: a URL, and one not asked about yet. */
    fun startAsking(server: String): Boolean = server.isNotBlank() && asked.add(server.trim())

    fun answered(server: String, offered: Boolean) {
        answers = answers + (server.trim() to offered)
    }

    /** A press: whatever flight was in the air is stopped. Names the new attempt. */
    fun begin(): Long {
        attempt += 1
        flight?.stop()
        flight = null
        signingIn = true
        return attempt
    }

    /** [flight] carries [attempt]; one for an attempt already replaced is stopped. */
    fun flying(attempt: Long, flight: NextcloudFlight) {
        if (attempt == this.attempt) this.flight = flight else flight.stop()
    }

    fun current(attempt: Long): Boolean = attempt == this.attempt

    /** [attempt] ended. False when it was replaced meanwhile, and then nothing it found counts. */
    fun ended(attempt: Long): Boolean {
        if (attempt != this.attempt) return false
        signingIn = false
        flight = null
        return true
    }

    /** The app is in front again: the button stops spinning, and the flight still takes a grant. */
    fun resumed() {
        signingIn = false
    }

    fun hold(server: String, grant: String) {
        held = server.trim() to grant
    }

    /** The grant kept for [server], when there is one. */
    fun heldFor(server: String): String? = held?.takeIf { it.first == server.trim() }?.second

    /** Takes the kept grant: it became an account, or is to be withdrawn. */
    fun take(): String? = held?.second.also { held = null }

    /** Setup was left: nothing in the air may act for it. Returns a kept grant, to withdraw. */
    fun leave(): String? {
        attempt += 1
        flight?.stop()
        flight = null
        signingIn = false
        answers = emptyMap()
        asked.clear()
        return take()
    }
}
