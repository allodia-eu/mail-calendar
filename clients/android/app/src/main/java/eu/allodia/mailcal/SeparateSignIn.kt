// A calendar or address book given a sign-in of its own (docs/accounts.md rule 18): the form open,
// whether its connect runs, whether its server offers a sign-in at its own provider
// (docs/mail-oauth.md rule 13), and what the last attempt said. One for setup's step and one for
// the account page's dialog, each held by the activity; Compose-free in its decisions.
package eu.allodia.mailcal

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue

/**
 * What the form's controls do. Only the blocking core calls disable anything: a connect or a skip
 * holds the whole form, the sign-in's start holds all but the way back, and its completion holds
 * everything. While the browser has the sign-in, a fresh press of its button supersedes it.
 */
internal data class SeparateControls(
    /** Connect's spinner: a connect or a skip runs. */
    val connecting: Boolean = false,
    /** Whether the sign-in at the server's own provider, or Nextcloud's, is drawn. */
    val offersSignIn: Boolean = false,
    /** The sign-in drawn is Nextcloud's (docs/nextcloud-sign-in.md). */
    val nextcloud: Boolean = false,
    /** The sign-in button's spinner. */
    val signingIn: Boolean = false,
    /** The fields, Connect and the sign-in button. */
    val editable: Boolean = true,
    /** Skip, Cancel and back. */
    val leavable: Boolean = true,
) {
    companion object {
        fun of(
            busy: Boolean,
            offersSignIn: Boolean,
            step: SeparateSignInStep?,
            nextcloud: Boolean = false,
        ): SeparateControls {
            val completing = step == SeparateSignInStep.COMPLETING
            return SeparateControls(
                connecting = busy,
                offersSignIn = offersSignIn || nextcloud,
                nextcloud = nextcloud,
                signingIn = step != null,
                editable = !busy && step != SeparateSignInStep.STARTING && !completing,
                leavable = !busy && !completing,
            )
        }
    }
}

internal class SeparateSignIn {
    /** The form on screen, or null. */
    var form by mutableStateOf<SeparateServerForm?>(null)
        private set

    /** Whether its connect, or Skip's switching off, runs; the controls hold still meanwhile. */
    var busy by mutableStateOf(false)
        private set

    /** Why the last connect or sign-in failed, said under the fields. */
    var error by mutableStateOf<String?>(null)
        private set

    /** Whether the core said the form's server signs in at its own provider. */
    var offersSignIn by mutableStateOf(false)
        private set

    /** Whether the core said the form's server is Nextcloud, whose own sign-in it offers. */
    var offersNextcloud by mutableStateOf(false)
        private set

    /**
     * A Nextcloud grant no account was made of yet, kept so the next press tries it again without
     * the browser: the server hands it out once.
     */
    var grant: String? = null

    /** The Nextcloud sign-in in the air, stopped when the form moves on from it. */
    var flight: NextcloudFlight? = null

    /** The browser sign-in, while one runs or its redirect is awaited. */
    val browser = SeparateServerSignInSlot()

    private var asked: SeparateServerForm? = null

    val controls: SeparateControls get() = SeparateControls.of(busy, offersSignIn, browser.step, offersNextcloud)

    fun open(form: SeparateServerForm) {
        close()
        this.form = form
    }

    /** Whether to ask the core about [form] now: once for each opening, while it is on screen. */
    fun startAsking(form: SeparateServerForm): Boolean {
        if (this.form != form || asked == form) return false
        asked = form
        return true
    }

    /**
     * The core answered for [form]: whether it signs in at its own provider, or else with
     * Nextcloud. An answer for a form no longer on screen is dropped.
     */
    fun answered(form: SeparateServerForm, offers: Boolean, nextcloud: Boolean = false) {
        if (this.form != form) return
        offersSignIn = offers
        offersNextcloud = nextcloud && !offers
    }

    /**
     * Starts a connect or a skip; false while one runs, while a sign-in completes, or with no form
     * open. A sign-in still out is superseded, so its redirect adds nothing.
     */
    fun start(): Boolean {
        if (form == null || busy || browser.step == SeparateSignInStep.COMPLETING) return false
        retire()
        busy = true
        error = null
        return true
    }

    /** Starts the browser sign-in, superseding one still out: the attempt, or null when it may not. */
    fun beginSignIn(): Long? {
        if (form == null || !controls.offersSignIn || !controls.editable) return null
        error = null
        return browser.begin()
    }

    /** The connect or the sign-in failed with [said]; the form stays. */
    fun failed(said: String) {
        busy = false
        error = said
    }

    /**
     * The form left the screen: a sign-in still out for it is retired. One completing is not, as
     * its account is already being added.
     */
    fun left() {
        if (browser.step != SeparateSignInStep.COMPLETING) retire()
    }

    /** The form goes, and with it any sign-in still running for it. */
    fun close() {
        form = null
        busy = false
        error = null
        offersSignIn = false
        offersNextcloud = false
        asked = null
        retire()
    }

    /**
     * Retires the sign-in in the air, stopping a Nextcloud flight and withdrawing a kept grant;
     * never one the core is making an account of now.
     */
    fun retire() {
        val completing = browser.step == SeparateSignInStep.COMPLETING
        browser.retire()
        flight?.stop()
        flight = null
        if (!completing) {
            NextcloudFlights.withdraw(grant)
            grant = null
        }
    }
}
