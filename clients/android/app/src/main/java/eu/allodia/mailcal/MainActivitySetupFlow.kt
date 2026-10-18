// What follows a connect on the setup screen (docs/onboarding.md, "After the connect"), driven from
// the activity because every route that adds an account ends here, a browser sign-in included: the
// uses a JMAP server offers, a calendar on a server of its own, the link step, "Add another account", the name each added account is
// asked once the flow ends, and what a provider withheld, said on the account's page after that.
package eu.allodia.mailcal

import kotlin.concurrent.thread
import uniffi.mailcal_bindings.AccountCapability
import uniffi.mailcal_bindings.AccountKind as CoreAccountKind
import uniffi.mailcal_bindings.CapabilityChange
import uniffi.mailcal_bindings.offersSetupLinks

/** The link step for [account], from the snapshot last read. */
internal fun MainActivity.linkStepFor(account: String): SetupLinkStep? =
    SetupLinkStep.of(accountSettings.snapshot, account, ::offersSetupLinks, setupFlow.besideFor(account))

/**
 * An account was added. A sign-in at the provider may have been granted less than was chosen, which
 * is said once setup and its name prompts are done ([signedInAtProvider]); a calendar or address
 * book whose server needs a sign-in of its own asks for it next, as the add's dial found it.
 */
internal fun MainActivity.afterAccountAdded(account: String, signedInAtProvider: Boolean = false) {
    val instance = app ?: return
    refreshAccountsSnapshot()
    val entry = accountSettings.entry(account)
    if (signedInAtProvider) {
        entry?.let(AccountSettingsRules::withheld)?.takeIf { it.isNotEmpty() }?.let {
            setupFlow.withheld = account to it
        }
    }
    // A connect or browser sign-in that finished after setup was cancelled: the account is added,
    // so it is still asked its name, but no step opens behind a screen that has gone.
    if (!needsSetup && !addingAccount) {
        setupFlow.namesWaiting.addLast(account)
        if (senderNamePrompt == null && !setupFlow.askingName) askNextSenderName()
        return
    }
    val choices = instance.signedInSetupChoices(account)
    val goesOn = setupFlow.accountAdded(
        account,
        jmap = entry?.kind == CoreAccountKind.JMAP,
        choices = choices,
        stepFor = ::linkStepFor,
        separate = UnreachedRules.form(entry),
    )
    if (!goesOn) finishSetup()
}

/**
 * The uses step answered: what was chosen is switched on and what was not is switched off, which
 * deletes, so off this thread.
 */
internal fun MainActivity.chooseSetupUses(chosen: List<AccountCapability>) {
    val step = setupFlow.uses ?: return
    val instance = app
    val changes = step.changes(chosen)
    if (instance == null || changes.isEmpty()) {
        if (!setupFlow.usesChosen(::linkStepFor)) finishSetup()
        return
    }
    thread(name = "mailcal-setup-uses") {
        changes.forEach { (capability, on) ->
            runCatching { instance.setAccountCapability(step.account, capability, on) }
                .onSuccess { if (it != CapabilityChange.APPLIED) logUiWarn("setup: a use was not switched on: $it") }
                .onFailure { logUiWarn("setup: a use could not be switched: ${coreErrorText(it)}") }
        }
        mainHandler.post {
            refreshAccountsSnapshot()
            if (!setupFlow.usesChosen(::linkStepFor)) finishSetup()
        }
    }
}

/** The link step's Continue or Skip: the picks are linked only on Continue. */
internal fun MainActivity.finishLinkStep(link: Boolean) {
    val step = setupFlow.links
    if (link && step != null) {
        step.links().forEach { (slot, target) -> setAccountLink(step.account, slot, target) }
    }
    finishSetup()
}

/** "Add another account": setup again from the address, coming back to the link step. */
internal fun MainActivity.addLinkedAccount() {
    clearSetupForm()
    // The new form opens on the address, not on the offer this flow may have started from.
    setupStartEmail = ""
    setupStartOffer = null
    setupFlow.addLinked()
}

/** The link step's offer of the servers found beside its account's own: setup on those. */
internal fun MainActivity.setUpBeside() {
    val instance = app ?: return
    val account = setupFlow.links?.account ?: return
    val beside = setupFlow.besideFor(account) ?: return
    val setup = instance.davSetupBeside(beside.account, beside.email, beside.caldavUrl, beside.carddavUrl)
        ?: return
    clearSetupForm()
    setupStartEmail = ""
    setupStartOffer = null
    setupFlow.addBeside()
    setupFlow.startOn = beside.email to setup
}

/**
 * Cancel, or back from the address: back to the link step an "Add another account" left, or the
 * end of the flow. A flow that added an account ends as a skipped link step does, so each account
 * added is still asked its name.
 */
internal fun MainActivity.cancelSetup() {
    clearSetupForm()
    if (setupFlow.cancelled(::linkStepFor)) return
    if (setupFlow.addedAny) {
        finishSetup()
    } else {
        setupFlow.finish()
        setupStartEmail = ""
        setupStartOffer = null
        addingAccount = false
    }
}

/**
 * What one setup form left behind: its error, a refused certificate and its acceptance, and the
 * offer to update an account already set up.
 */
private fun MainActivity.clearSetupForm() {
    addError = null
    addFailure = null
    setupAcceptedCertificate = null
    setupFlow.existing = null
}

/**
 * The account already set up took the credential the form was given. Nothing was added, so the
 * flow goes back to the link step it was opened from, or ends.
 */
internal fun MainActivity.endSetupAfterUpdate() {
    clearSetupForm()
    if (!setupFlow.cancelled(::linkStepFor)) finishSetup()
}

/** The end of the flow: the setup screen goes, and each account added is asked its name in turn. */
internal fun MainActivity.finishSetup() {
    val added = setupFlow.finish()
    clearSetupForm()
    setupStartEmail = ""
    setupStartOffer = null
    needsSetup = false
    addingAccount = false
    setupFlow.namesWaiting.addAll(added)
    if (senderNamePrompt == null && !setupFlow.askingName) askNextSenderName()
}

/**
 * Asks the next waiting account its name, where the core says it still needs one: a provider
 * that already holds one has had it adopted, and an account without mail sends nothing
 * (docs/sending.md). A provider round trip, so off this thread; a provider that cannot be asked
 * means ask. Once none is waiting, what a sign-in withheld is said on that account's page.
 */
internal fun MainActivity.askNextSenderName() {
    senderNamePrompt = null
    val instance = app ?: return
    val next = setupFlow.namesWaiting.removeFirstOrNull()
    if (next == null) {
        setupFlow.withheld?.let { (account, uses) ->
            setupFlow.withheld = null
            openAccountPage(account)
            accountSettings.notice = AccountNotice(
                NoticeKind.WARNING,
                L10n.setup_withheld_title(this),
                L10n.setup_withheld_detail(this, uses.joinToString(", ") { useName(this, it) }),
            )
        }
        return
    }
    // One account is asked at a time: a second end of the flow meanwhile only adds to the queue.
    setupFlow.askingName = true
    thread(name = "mailcal-sender-name") {
        val needed = runCatching { instance.needsSenderName(next) }.getOrElse {
            logUiWarn("sender name: the provider was not asked: ${coreErrorText(it)}")
            true
        }
        mainHandler.post {
            setupFlow.askingName = false
            if (needed) senderNamePrompt = next else askNextSenderName()
        }
    }
}
