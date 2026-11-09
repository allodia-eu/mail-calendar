// A calendar or address book given a sign-in of its own (docs/accounts.md rule 18): the form open,
// drawn from the core's section, and the sign-in in the browser this client still runs
// (docs/mail-oauth.md rule 13) with what it last said. One for setup's step and one for the account
// page's dialog, each held by the activity; Compose-free in its decisions.
package eu.allodia.mailcal

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import uniffi.mailcal_bindings.AccountSeparateServer
import uniffi.mailcal_bindings.AccountSeparateSignIn
import uniffi.mailcal_bindings.ControlState

/**
 * What the form's controls do. Connect and Skip are the core's controls, held as well while the
 * sign-in's own blocking core calls run: its start holds all but the way back, and its completion
 * holds everything. While the browser has the sign-in, a fresh press of its button supersedes it.
 */
internal data class SeparateControls(
    /** Connect, as the core draws it. */
    val connect: ControlState,
    /** Setup's Skip, as the core draws it. */
    val skip: ControlState,
    /** Whether the sign-in at the server's own provider, or Nextcloud's, is drawn. */
    val offersSignIn: Boolean = false,
    /** The sign-in drawn is Nextcloud's (docs/nextcloud-sign-in.md). */
    val nextcloud: Boolean = false,
    /** The sign-in button's spinner. */
    val signingIn: Boolean = false,
    /** The fields and the sign-in button. */
    val editable: Boolean = true,
    /** The dialog's Cancel and setup's back. */
    val leavable: Boolean = true,
) {
    companion object {
        fun of(section: AccountSeparateServer, step: SeparateSignInStep?): SeparateControls {
            val completing = step == SeparateSignInStep.COMPLETING
            val starting = step == SeparateSignInStep.STARTING
            val running = section.connect.running || section.stopUsing.running
            return SeparateControls(
                connect = section.connect.copy(enabled = section.connect.enabled && !starting && !completing),
                skip = section.stopUsing.copy(enabled = section.stopUsing.enabled && !completing),
                offersSignIn = section.signIn != null,
                nextcloud = section.signIn == AccountSeparateSignIn.NEXTCLOUD,
                signingIn = step != null,
                editable = section.connect.enabled && !starting && !completing,
                leavable = !running && !completing,
            )
        }
    }
}

internal class SeparateSignIn {
    /** The form on screen, or null. */
    var form by mutableStateOf<SeparateServerForm?>(null)
        private set

    /** Why the last sign-in in the browser failed; a Connect's failure is the section's. */
    var error by mutableStateOf<String?>(null)
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

    val controls: SeparateControls?
        get() = form?.let { SeparateControls.of(it.section, browser.step) }

    /** Whether the server's own sign-in is Nextcloud's. */
    val offersNextcloud: Boolean get() = form?.section?.signIn == AccountSeparateSignIn.NEXTCLOUD

    fun open(form: SeparateServerForm) {
        close()
        this.form = form
    }

    /**
     * The core drew the form's section again: [section], or null once nothing needs the sign-in.
     * False when the form is over, which the caller goes on from.
     */
    fun redraw(section: AccountSeparateServer?): Boolean {
        val open = form ?: return true
        if (section == null) return false
        if (section != open.section) form = open.copy(section = section)
        return true
    }

    /**
     * About to send Connect or Skip: false while the core or a completing sign-in holds them, or
     * with no form open. A sign-in still out is superseded, so its redirect adds nothing.
     */
    fun sending(skip: Boolean = false): Boolean {
        val controls = controls ?: return false
        if (!(if (skip) controls.skip.enabled else controls.connect.enabled)) return false
        retire()
        error = null
        return true
    }

    /** Starts the browser sign-in, superseding one still out: the attempt, or null when it may not. */
    fun beginSignIn(): Long? {
        val controls = controls ?: return null
        if (!controls.offersSignIn || !controls.editable) return null
        error = null
        return browser.begin()
    }

    /** The sign-in failed with [said]; the form stays. */
    fun failed(said: String) {
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
        error = null
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
