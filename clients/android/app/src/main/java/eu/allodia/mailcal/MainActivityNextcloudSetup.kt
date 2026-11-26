// Signing in with Nextcloud from setup (docs/nextcloud-sign-in.md rule 2): the found card for a
// calendar and contacts server detection recognised as Nextcloud, the same card set up from beside
// a JMAP server, the manual calendar and contacts form once the server typed there answers as
// Nextcloud, and the manual form's Nextcloud pane, from the server's address alone. What is added is the calendar and contacts account the grant signs in to. Every core
// call here blocks, so each runs off the main thread.
package eu.allodia.mailcal

import kotlin.concurrent.thread
import uniffi.mailcal_bindings.AccountCapability
import uniffi.mailcal_bindings.MailcalApp
import uniffi.mailcal_bindings.MailcalException
import uniffi.mailcal_bindings.NextcloudAddressStart
import uniffi.mailcal_bindings.NextcloudSignInStart

/** What the setup cards draw of the sign-in, and what their buttons send. */
internal fun MainActivity.nextcloudSetupUi(): NextcloudSetupUi = NextcloudSetupUi(
    signingIn = setupFlow.nextcloud.signingIn,
    offers = setupFlow.nextcloud::offers,
    onAsk = ::askNextcloudOffer,
    onSignIn = ::signInWithNextcloud,
    checking = setupFlow.nextcloud.checking,
    unsureFor = setupFlow.nextcloud::unsureFor,
    onSignInAt = ::signInWithNextcloudAt,
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
internal fun MainActivity.signInWithNextcloud(server: String, uses: List<AccountCapability>) =
    startSetupNextcloud(server, uses, checking = false) { it.beginNextcloudSignIn(server) }

/**
 * Signs in at the address the Nextcloud pane holds, for calendar and contacts: once the core has
 * found Nextcloud there, or [anyway], after it could not tell. An address that starts no sign-in
 * ends the attempt saying why, or, when the core could not tell, offering the sign-in anyway.
 */
internal fun MainActivity.signInWithNextcloudAt(address: String, anyway: Boolean) {
    val server = address.trim()
    startSetupNextcloud(server, NEXTCLOUD_USES, checking = !anyway) { core ->
        nextcloudAddressStarted(
            if (anyway) core.beginNextcloudSignInAnyway(server) else core.beginNextcloudSignInAt(server),
        )
    }
}

private fun MainActivity.startSetupNextcloud(
    server: String,
    uses: List<AccountCapability>,
    checking: Boolean,
    begin: (MailcalApp) -> NextcloudSignInStart,
) {
    val state = setupFlow.nextcloud
    if (isConnecting) return
    addError = null
    addFailure = null
    setupFlow.existing = null
    state.heldFor(server)?.let { return addNextcloud(it, uses) }
    abandonNextcloud(state.take())
    val attempt = state.begin(checking)
    logUiInfo("setup: signing in with Nextcloud")
    val flight = flyNextcloud(
        "mailcal-nextcloud-setup",
        begin = begin,
        opening = { state.current(attempt).also { if (it) state.opened(attempt) } },
    ) { end ->
        if (!state.ended(attempt)) {
            if (end is NextcloudEnd.Granted) abandonNextcloud(end.grant)
            return@flyNextcloud
        }
        val refused = ((end as? NextcloudEnd.Failed)?.error as? NextcloudAddressRefused)?.answer
        when {
            end is NextcloudEnd.Granted -> {
                state.hold(server, end.grant)
                addNextcloud(end.grant, uses)
            }
            refused != null -> {
                if (refused is NextcloudAddressStart.Unsure) state.unsure(attempt, server, refused.host)
                addError = nextcloudRefusalSaid(refused, this)
            }
            else -> addError = nextcloudSaid(end, this)
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
