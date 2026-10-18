// A calendar or address book given a sign-in of its own (docs/accounts.md rule 18): the form open,
// whether its connect runs, and what the last one said. One for setup's step and one for the
// account page's dialog, each held by the activity; Compose-free in its decisions.
package eu.allodia.mailcal

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue

internal class SeparateSignIn {
    /** The form on screen, or null. */
    var form by mutableStateOf<SeparateServerForm?>(null)
        private set

    /** Whether its connect, or Skip's switching off, runs; the controls hold still meanwhile. */
    var busy by mutableStateOf(false)
        private set

    /** Why the last connect failed, said under the fields. */
    var error by mutableStateOf<String?>(null)
        private set

    fun open(form: SeparateServerForm) {
        this.form = form
        busy = false
        error = null
    }

    /** Starts a connect or a skip; false while one runs or no form is open. */
    fun start(): Boolean {
        if (form == null || busy) return false
        busy = true
        error = null
        return true
    }

    /** The connect failed with [said]; the form stays. */
    fun failed(said: String) {
        busy = false
        error = said
    }

    fun close() {
        form = null
        busy = false
        error = null
    }
}
