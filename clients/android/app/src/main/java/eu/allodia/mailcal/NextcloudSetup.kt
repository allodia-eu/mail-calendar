// What setup knows about a sign-in with Nextcloud (docs/nextcloud-sign-in.md rules 2 and 6): the
// servers the manual form asked about, the attempt in the air, an address the Nextcloud pane could
// not place, and a grant no account was made of yet, kept so the next press tries it again
// without the browser. Compose-free in its decisions,
// so the JVM suite drives them.
package eu.allodia.mailcal

import android.content.Context
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import uniffi.mailcal_bindings.AccountCapability
import uniffi.mailcal_bindings.NextcloudAddressStart
import uniffi.mailcal_bindings.NextcloudSignInStart

/** What an account signed in to from the Nextcloud pane is used for: Nextcloud serves both. */
internal val NEXTCLOUD_USES = listOf(AccountCapability.CALENDAR, AccountCapability.CONTACTS)

/** An address typed as a Nextcloud server's at which no sign-in started, and what it answered. */
internal class NextcloudAddressRefused(val answer: NextcloudAddressStart) : Exception("no sign-in started")

/** The sign-in a typed address started; any other answer is thrown as the refusal that says why. */
internal fun nextcloudAddressStarted(answer: NextcloudAddressStart): NextcloudSignInStart =
    (answer as? NextcloudAddressStart.Started)?.start ?: throw NextcloudAddressRefused(answer)

/** What the pane says of an address that started no sign-in; null when it was unsure, which the pane draws itself. */
internal fun nextcloudRefusalSaid(answer: NextcloudAddressStart, ctx: Context): String? = when (answer) {
    is NextcloudAddressStart.NotNextcloud -> L10n.setup_nextcloud_not_found(ctx, host = answer.host)
    is NextcloudAddressStart.Maintenance -> L10n.setup_nextcloud_maintenance(ctx, host = answer.host)
    NextcloudAddressStart.NotAnAddress -> L10n.setup_nextcloud_address_invalid(ctx)
    is NextcloudAddressStart.Unsure, is NextcloudAddressStart.Started -> null
}

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
    /** The Nextcloud pane's server is being asked whether it is Nextcloud, before any browser. */
    val checking: Boolean = false,
    /** The host to offer the sign-in anyway at, while the pane holds the address it could not place. */
    val unsureFor: (String) -> String? = { null },
    /** Signs in at the address the Nextcloud pane holds: asked first, or `anyway` once it was unsure. */
    val onSignInAt: (address: String, anyway: Boolean) -> Unit = { _, _ -> },
)

internal class SetupNextcloud {
    var signingIn by mutableStateOf(false)
        private set

    /** The current attempt's server is being asked whether it is Nextcloud. */
    var checking by mutableStateOf(false)
        private set

    /** The address the pane asked about that showed nothing either way, and the host it named. */
    private var unsure by mutableStateOf<Pair<String, String>?>(null)

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

    /** The host to offer the sign-in anyway at, while the pane holds the address it asked. */
    fun unsureFor(typed: String): String? = unsure?.takeIf { it.first == typed.trim() }?.second

    /** [address] showed nothing either way, naming [host]; only for the attempt still current. */
    fun unsure(attempt: Long, address: String, host: String) {
        if (attempt == this.attempt) unsure = address.trim() to host
    }

    /**
     * A press: whatever flight was in the air is stopped, and an offer to sign in anyway is
     * answered. [checking] when the server is asked whether it is Nextcloud before the browser.
     * Names the new attempt.
     */
    fun begin(checking: Boolean = false): Long {
        attempt += 1
        flight?.stop()
        flight = null
        signingIn = true
        this.checking = checking
        unsure = null
        return attempt
    }

    /** [attempt]'s server answered with a sign-in: the browser opens on it. */
    fun opened(attempt: Long) {
        if (attempt == this.attempt) checking = false
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
        checking = false
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
        checking = false
        unsure = null
        answers = emptyMap()
        asked.clear()
        return take()
    }
}
