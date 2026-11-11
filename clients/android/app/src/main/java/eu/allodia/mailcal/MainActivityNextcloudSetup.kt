// Signing in with Nextcloud from setup (docs/nextcloud-sign-in.md rule 2): the found card for a
// calendar and contacts server detection recognised as Nextcloud, the same card set up from beside
// a JMAP server, and the manual calendar and contacts form once the server typed there answers as
// Nextcloud. What is added is the calendar and contacts account the grant signs in to. Every core
// call here blocks, so each runs off the main thread.
package eu.allodia.mailcal

import kotlin.concurrent.thread
import uniffi.mailcal_bindings.AccountCapability
import uniffi.mailcal_bindings.MailcalException

/** What the setup cards draw of the sign-in, and what their buttons send. */
internal fun MainActivity.nextcloudSetupUi(): NextcloudSetupUi = NextcloudSetupUi(
    signingIn = setupFlow.nextcloud.signingIn,
    offers = setupFlow.nextcloud::offers,
    onAsk = ::askNextcloudOffer,
    onSignIn = ::signInWithNextcloud,
)

/** Asks whether the server at the URL the manual form names is Nextcloud. Never an error. */
internal fun MainActivity.askNextcloudOffer(server: String) {
    val instance = app ?: return
    val state = setupFlow.nextcloud
    if (!state.startAsking(server)) return
    val asked = server.trim()
    thread(name = "mailcal-nextcloud-offer") {
        val offered = runCatching { instance.offersNextcloudSignIn(asked) }.getOrDefault(false)
        mainHandler.post { state.answered(asked, offered) }
    }
}

/**
 * Signs in at [server] for [uses], superseding a sign-in still in the air. A grant kept from the
 * last try goes straight to the core.
 */
internal fun MainActivity.signInWithNextcloud(server: String, uses: List<AccountCapability>) {
    val state = setupFlow.nextcloud
    if (isConnecting) return
    addError = null
    addFailure = null
    setupFlow.existing = null
    state.heldFor(server)?.let { return addNextcloud(it, uses) }
    abandonNextcloud(state.take())
    val attempt = state.begin()
    logUiInfo("setup: signing in with Nextcloud")
    val flight = flyNextcloud(
        "mailcal-nextcloud-setup",
        begin = { it.beginNextcloudSignIn(server) },
        opening = { state.current(attempt) },
    ) { end ->
        if (!state.ended(attempt)) {
            if (end is NextcloudEnd.Granted) abandonNextcloud(end.grant)
            return@flyNextcloud
        }
        if (end is NextcloudEnd.Granted) {
            state.hold(server, end.grant)
            addNextcloud(end.grant, uses)
        } else {
            addError = nextcloudSaid(end, this)
        }
    }
    if (flight == null) state.ended(attempt) else state.flying(attempt, flight)
}

/**
 * Adds the account [grant] signs in to, used for [uses], and goes on as an add does. An account
 * already set up for this person is offered the grant instead.
 */
private fun MainActivity.addNextcloud(grant: String, uses: List<AccountCapability>) {
    val instance = app ?: return
    isConnecting = true
    thread(name = "mailcal-nextcloud-add") {
        val answer = runCatching { instance.addNextcloudAccount(grant, uses) }
        mainHandler.post {
            isConnecting = false
            answer.onSuccess { row ->
                setupFlow.nextcloud.take()
                afterAccountAdded(row.id)
            }
            answer.onFailure { error ->
                logUiWarn("setup: the Nextcloud account was not added: ${coreErrorText(error)}")
                reportAddFailure(error)
                if (error is MailcalException.AlreadySetUp) {
                    setupFlow.existing = ExistingAccount(error.accountId, "", nextcloudGrant = grant)
                }
            }
        }
    }
}

/** Setup was left: a sign-in in the air stops, and a kept grant is withdrawn. */
internal fun MainActivity.leaveSetupNextcloud() {
    abandonNextcloud(setupFlow.nextcloud.leave())
}
