// What follows a connect on the setup screen (docs/onboarding.md, "After the connect"): the uses a
// JMAP server offers, the link step, the accounts an "Add another account" adds on the way, which of
// them are asked their name once the flow ends, and what a sign-in's grant withheld, said after
// that. Held by the activity, because a browser sign-in leaves the app mid-flow; Compose-free in
// its decisions, so the JVM suite drives them.
package eu.allodia.mailcal

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import uniffi.mailcal_bindings.AccountCapability
import uniffi.mailcal_bindings.DetectedSetup
import uniffi.mailcal_bindings.SetupChoice

/**
 * The step a sign-in ends on when only the server could say what the account offers: a JMAP session
 * offering more than one of mail, calendar and contacts. [next] is the account whose link step
 * follows: the one an "Add another account" left, or this one.
 */
internal data class UsesStep(val account: String, val next: String, val offer: UseOffer) {
    /**
     * What [chosen] switches, as (use, on): each use chosen that is off now, then each use on now
     * and not chosen. On first, so the account never passes through being used for nothing.
     */
    fun changes(chosen: List<AccountCapability>): List<Pair<AccountCapability, Boolean>> {
        val on = offer.choices.filter { !it.on && it.capability in chosen }.map { it.capability to true }
        val off = offer.choices.filter { it.on && it.capability !in chosen }.map { it.capability to false }
        return on + off
    }

    companion object {
        /** The step for [account], or null when the core offers nothing to choose. */
        fun of(account: String, next: String, choices: List<SetupChoice>): UsesStep? =
            choices.takeIf { it.isNotEmpty() }?.let { UsesStep(account, next, UseOffer(it)) }
    }
}

/** The calendar and address-book servers a detection found beside a JMAP server, for [email]. */
internal data class FoundBeside(val email: String, val caldavUrl: String?, val carddavUrl: String?) {
    fun offerFor(account: String) = Beside(account, email, caldavUrl, carddavUrl)

    companion object {
        /** What a JMAP route's detection found beside its server, if anything. */
        fun of(email: String, setup: DetectedSetup): FoundBeside? =
            setup.takeIf {
                it.recommendation is uniffi.mailcal_bindings.SetupRecommendation.Jmap &&
                    (it.caldavUrl != null || it.carddavUrl != null)
            }?.let { FoundBeside(email, it.caldavUrl, it.carddavUrl) }
    }
}

internal class SetupFlow {
    /** The link step, kept while "Add another account" runs so what was picked comes back. */
    var links by mutableStateOf<SetupLinkStep?>(null)
        private set

    /** The uses step on screen, ahead of everything else. */
    var uses by mutableStateOf<UsesStep?>(null)
        private set

    /** The account whose link step "Add another account" left, to come back to. */
    var returningTo by mutableStateOf<String?>(null)
        private set

    /** Bumped to draw the setup form afresh, from the address, for another account. */
    var form by mutableIntStateOf(0)
        private set

    /** A route to open the form on, rather than the address: the servers found beside a server. */
    var startOn by mutableStateOf<Pair<String, DetectedSetup>?>(null)

    /** The servers detection found beside the JMAP card on screen, kept past "Set up manually". */
    var found: FoundBeside? = null

    private val added = mutableListOf<String>()
    private val beside = mutableListOf<Beside>()

    /** The accounts a finished flow added, waiting to be asked their name one after another. */
    val namesWaiting = ArrayDeque<String>()

    /** Whether a name lookup is running, so a second end of the flow does not start another. */
    var askingName = false

    /** What a provider withheld from an account the flow added, said once the names are asked. */
    var withheld: Pair<String, List<AccountCapability>>? = null

    /** Whether the link step is what the screen shows. */
    val showsLinks: Boolean get() = uses == null && returningTo == null && links != null

    /** Whether an account has been added in this flow, so leaving it still asks their names. */
    val addedAny: Boolean get() = added.isNotEmpty()

    /** The servers found beside [account]'s own, while unused. */
    fun besideFor(account: String): Beside? = beside.firstOrNull { it.account == account }

    /**
     * An account was added: which of the uses its server offers it is used for, when only the
     * server could say ([choices]); then back to the link step it was added from, on to its own
     * when another account can fill something it lacks, or the end of the flow.
     *
     * @return whether the flow goes on; false means it ends now.
     */
    fun accountAdded(
        account: String,
        jmap: Boolean,
        choices: List<SetupChoice>,
        stepFor: (String) -> SetupLinkStep?,
    ): Boolean {
        added += account
        if (jmap) found?.let { beside += it.offerFor(account) }
        found = null
        startOn = null
        // A connect still running when "Add another account" was cancelled finishes onto the link
        // step that cancel returned to.
        val next = returningTo ?: links?.account ?: account
        returningTo = null
        UsesStep.of(account, next, choices)?.let {
            uses = it
            return true
        }
        return onward(account, next, stepFor)
    }

    /** The uses step answered: on to its link step, or the end. */
    fun usesChosen(stepFor: (String) -> SetupLinkStep?): Boolean {
        val step = uses ?: return false
        uses = null
        return onward(step.account, step.next, stepFor)
    }

    /** On to [next]'s link step, with [added] picked on it when it was added from it. */
    private fun onward(added: String, next: String, stepFor: (String) -> SetupLinkStep?): Boolean {
        val fresh = stepFor(next)
        if (fresh == null) {
            links = null
            return false
        }
        val step = links?.takeIf { it.account == fresh.account }?.refreshed(fresh) ?: fresh
        links = if (added != next) step.adopt(added) else step
        return true
    }

    /** Records a pick on the link step. */
    fun pick(picker: Int, option: Int?) {
        links = links?.pick(picker, option)
    }

    /** "Add another account": setup again from the address, coming back to this step. */
    fun addLinked() {
        val step = links ?: return
        returningTo = step.account
        form += 1
    }

    /** The link step's offer of the servers found beside its account, used up as it is taken. */
    fun addBeside(): Beside? {
        val account = links?.account ?: return null
        val offer = besideFor(account) ?: return null
        beside.remove(offer)
        addLinked()
        return offer
    }

    /**
     * Cancel, or back from the first screen: back to the link step an "Add another account" left.
     *
     * @return whether the flow goes on; false means it ends, and a flow that added an account ends
     *   as a skipped link step does, so each account added is still asked its name.
     */
    fun cancelled(stepFor: (String) -> SetupLinkStep?): Boolean {
        uses?.let { return usesChosen(stepFor) }
        val account = returningTo ?: return false
        returningTo = null
        startOn = null
        return onward(account, account, stepFor)
    }

    /** A suggestion the core found after the step was drawn reaches it, unless it was changed. */
    fun refresh(fresh: SetupLinkStep?) {
        val step = links ?: return
        if (fresh == null || fresh.account != step.account || returningTo != null) return
        links = step.refreshed(fresh)
    }

    /** The end of the flow: the accounts to ask a name for, in the order added. */
    fun finish(): List<String> {
        val names = added.toList()
        added.clear()
        beside.clear()
        links = null
        uses = null
        returningTo = null
        startOn = null
        found = null
        return names
    }
}
