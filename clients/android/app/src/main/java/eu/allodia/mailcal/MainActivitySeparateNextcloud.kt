// A separate calendar or address book server that is Nextcloud signs in with Nextcloud, from the
// form that otherwise asks for its password (docs/nextcloud-sign-in.md rule 2). The form's slot
// carries it through the provider sign-in's steps, so it draws the same way: starting while the
// core asks the server, the browser while the flight polls, completing while the core makes the
// account. A grant the core could not make an account of stays on the form for the next press.
package eu.allodia.mailcal

import kotlin.concurrent.thread

// What the slot holds while a Nextcloud flight polls: there is no redirect for it to complete.
private const val NEXTCLOUD_HANDLE = "nextcloud"

/** Starts the form's sign-in with Nextcloud, superseding one still in the air. */
internal fun MainActivity.signInSeparateNextcloud(state: SeparateSignIn) {
    val form = state.form ?: return
    state.flight?.stop()
    state.flight = null
    val attempt = state.beginSignIn() ?: return
    logUiInfo("accounts: signing a calendar or address book in with Nextcloud")
    state.grant?.let { grant ->
        state.browser.opening(attempt, NEXTCLOUD_HANDLE)
        state.browser.redirected()
        return addSeparateNextcloud(state, attempt, form, grant)
    }
    var mine: NextcloudFlight? = null
    mine = flyNextcloud(
        "mailcal-separate-nextcloud",
        begin = { it.beginSeparateServerNextcloudSignIn(form.accountId, form.uses) },
        opening = { state.browser.opening(attempt, NEXTCLOUD_HANDLE) },
    ) { end ->
        if (mine == null || state.flight !== mine) {
            if (end is NextcloudEnd.Granted) abandonNextcloud(end.grant)
            return@flyNextcloud
        }
        state.flight = null
        if (end is NextcloudEnd.Granted) {
            state.grant = end.grant
            state.browser.redirected()
            addSeparateNextcloud(state, attempt, form, end.grant)
        } else if (state.browser.settled(attempt)) {
            nextcloudSaid(end, this)?.let(state::failed)
        }
    }
    if (mine == null) state.browser.settled(attempt) else state.flight = mine
}

/** Moves the form's uses to the account [grant] signs in to, and goes on as Connect's success does. */
private fun MainActivity.addSeparateNextcloud(
    state: SeparateSignIn,
    attempt: Long,
    form: SeparateServerForm,
    grant: String,
) {
    val instance = app ?: return
    thread(name = "mailcal-separate-nextcloud-add") {
        val answer = runCatching { instance.addLinkedNextcloudAccount(form.accountId, form.uses, grant) }
        mainHandler.post {
            if (answer.isSuccess && state.grant == grant) state.grant = null
            if (!state.browser.settled(attempt)) {
                if (answer.isSuccess) separateAccountAdded()
                logUiInfo("accounts: the form moved on while a Nextcloud account was added")
                return@post
            }
            answer.onSuccess {
                logUiInfo("accounts: a calendar or address book now signs in with Nextcloud")
                separateLinked(state)
            }
            answer.onFailure {
                logUiWarn("accounts: the Nextcloud account was not added: ${coreErrorText(it)}")
                state.failed(separateSignInSaid(it, this))
            }
        }
    }
}
