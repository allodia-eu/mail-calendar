// The email-first setup flow's phase machine: which step is on screen, how a detection result
// routes to one, and what the manual form is prefilled with when the user edits a discovered
// config. Compose-free, and split out of AccountSetupDetect.kt to keep that file under the repo's
// 500-line limit.
package eu.allodia.mailcal

import uniffi.mailcal_bindings.DetectedSetup
import uniffi.mailcal_bindings.MissReason
import uniffi.mailcal_bindings.SetupRecommendation

// The flow's phases: type an email, wait, then either a routed card or the manual form.
internal sealed interface DetectPhase {
    object Email : DetectPhase
    object Detecting : DetectPhase

    // `signInOffered` and `imapAuth` are both resolved *before* this phase is entered, so the
    // card renders in its final shape. Resolving either afterwards meant the secret field
    // appeared for a second and then vanished under the user, the flash this design exists to
    // prevent, and it is the same reason `imapAuth` has no "still asking" state here: the
    // "Looking…" spinner is what somebody sees while the server is being asked.
    // `offer` is what the card offers the account for, as the core decided it.
    data class Found(
        val recommendation: SetupRecommendation,
        val signInOffered: Boolean,
        val imapAuth: ImapAuthState = ImapAuthState.Password,
        val offer: UseOffer = UseOffer.NONE,
    ) : DetectPhase

    // A domain with a calendar or address-book server and no mail server: an account used for
    // those alone, the typed address as its login.
    data class Dav(val email: String, val offer: UseOffer) : DetectPhase

    // `dav` is the calendar-and-contacts card's servers when that card sent the person here.
    data class Manual(
        val reason: MissReason?,
        val edit: SetupRecommendation?,
        val dav: UseOffer? = null,
    ) : DetectPhase
}

// Routes a detection result: a domain with only a calendar or address book to its own card, a
// Manual result to the manual form with its reason, and everything else to a routed card.
internal fun route(
    setup: DetectedSetup,
    email: String,
    signInOffered: Boolean,
    imapAuth: ImapAuthState = ImapAuthState.Password,
): DetectPhase {
    val recommendation = setup.recommendation
    return when {
        setup.calendarAndContacts -> DetectPhase.Dav(email.trim(), UseOffer.of(setup))
        recommendation is SetupRecommendation.Manual ->
            DetectPhase.Manual(reason = recommendation.reason, edit = null)
        else -> DetectPhase.Found(recommendation, signInOffered, imapAuth, UseOffer.of(setup))
    }
}

// A detection answer that offers no choice, for a route decided without one.
internal fun detected(recommendation: SetupRecommendation) =
    DetectedSetup(recommendation, false, null, null, emptyList())

// The localised line explaining why detection sent the user to manual setup.
internal fun reasonNote(reason: MissReason, ctx: android.content.Context): String = when (reason) {
    MissReason.NETWORK_ERROR -> L10n.setup_detect_reason_network(ctx)
    MissReason.OAUTH_ONLY_PROVIDER -> L10n.setup_detect_reason_oauth_only(ctx)
    MissReason.NOTHING_FOUND, MissReason.INVALID_EMAIL -> L10n.setup_detect_reason_nothing(ctx)
}

// What to prefill in the manual form when the user chooses to edit a discovered config.
internal data class ManualPrefill(
    val kind: AccountKind = AccountKind.PASSWORD,
    val email: String = "",
    val imapHost: String = "",
    val smtpHost: String = "",
    val jmapServer: String = "",
    val caldavUrl: String = "",
    val carddavUrl: String = "",
)

// The calendar-and-contacts card's servers carry into the manual form as that account type.
internal fun manualPrefill(edit: SetupRecommendation?, dav: UseOffer? = null): ManualPrefill = when {
    dav != null -> ManualPrefill(AccountKind.DAV, caldavUrl = dav.caldavUrl, carddavUrl = dav.carddavUrl)
    else -> manualPrefill(edit)
}

internal fun manualPrefill(edit: SetupRecommendation?): ManualPrefill = when (edit) {
    is SetupRecommendation.Imap ->
        ManualPrefill(AccountKind.PASSWORD, edit.email, edit.imapHost, edit.smtpHost ?: "", "")
    is SetupRecommendation.Jmap ->
        ManualPrefill(AccountKind.JMAP, edit.email, jmapServer = edit.serverUrl)
    is SetupRecommendation.Microsoft -> ManualPrefill(AccountKind.MICROSOFT, edit.email)
    is SetupRecommendation.Google -> ManualPrefill(AccountKind.GOOGLE, edit.email)
    else -> ManualPrefill()
}
