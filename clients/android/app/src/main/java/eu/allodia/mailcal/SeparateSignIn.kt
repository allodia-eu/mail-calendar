// A calendar or address book given a sign-in of its own (docs/accounts.md rule 18): the form open,
// drawn from the core's section. Connect, Skip and the sign-in in the browser, at the server's own
// provider (docs/mail-oauth.md rule 13) or with Nextcloud, are the core's, and so is what each
// came to. One for setup's step and one for the account page's dialog, each held by the activity;
// Compose-free in its decisions.
package eu.allodia.mailcal

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import uniffi.mailcal_bindings.AccountPageIntent
import uniffi.mailcal_bindings.AccountSeparateServer
import uniffi.mailcal_bindings.AccountSeparateSignIn
import uniffi.mailcal_bindings.BrowserSignIn
import uniffi.mailcal_bindings.BrowserSignInStage
import uniffi.mailcal_bindings.ControlState

/** What the form's controls do, read off the core's section. */
internal data class SeparateControls(
    /** Connect, as the core draws it. */
    val connect: ControlState,
    /** Setup's Skip, as the core draws it. */
    val skip: ControlState,
    /** The sign-in's button, as the core draws it; pressed while the browser waits, it starts afresh. */
    val signIn: ControlState,
    /** Whether the sign-in at the server's own provider, or Nextcloud's, is drawn. */
    val offersSignIn: Boolean,
    /** The sign-in drawn is Nextcloud's (docs/nextcloud-sign-in.md). */
    val nextcloud: Boolean,
    /** The sign-in waiting on the browser, drawn as a line with its Cancel. */
    val waiting: BrowserSignIn?,
    /** The fields, held as the core holds Connect. */
    val editable: Boolean,
    /** The dialog's Cancel and setup's back. */
    val leavable: Boolean,
) {
    companion object {
        fun of(section: AccountSeparateServer): SeparateControls = SeparateControls(
            connect = section.connect,
            skip = section.stopUsing,
            signIn = section.signInControl,
            offersSignIn = section.signIn != null,
            nextcloud = section.signIn == AccountSeparateSignIn.NEXTCLOUD,
            waiting = section.browser?.takeIf { it.stage is BrowserSignInStage.InBrowser },
            editable = section.connect.enabled,
            leavable = !section.connect.running && !section.stopUsing.running,
        )
    }
}

/**
 * What cancels [browser], the sign-in out for a form: its Cancel pressed, or the form leaving the
 * screen. Null while the core holds the Cancel, once the sign-in no longer waits on the person.
 */
internal fun separateCancel(browser: BrowserSignIn?): AccountPageIntent? =
    browser?.takeIf { it.cancel.enabled }?.let { AccountPageIntent.CancelSignIn(it.account, it.attempt) }

internal class SeparateSignIn {
    /** The form on screen, or null. */
    var form by mutableStateOf<SeparateServerForm?>(null)
        private set

    val controls: SeparateControls?
        get() = form?.let { SeparateControls.of(it.section) }

    fun open(form: SeparateServerForm) {
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

    /** About to send Connect or Skip: false while the core holds it, or with no form open. */
    fun sending(skip: Boolean = false): Boolean {
        val controls = controls ?: return false
        return if (skip) controls.skip.enabled else controls.connect.enabled
    }

    /** About to send the sign-in: false when none is offered, while the core holds it, or with no form open. */
    fun signingIn(): Boolean {
        val controls = controls ?: return false
        return controls.offersSignIn && controls.signIn.enabled
    }

    /** The form goes. A sign-in still out for it is cancelled as the form leaves the screen. */
    fun close() {
        form = null
    }
}
