// The email-first setup flow's phase machine: which step is on screen, how a detection result
// routes to one, and what the manual form is prefilled with when the user edits a discovered
// config. Compose-free, and split out of AccountSetupDetect.kt to keep that file under the repo's
// 500-line limit.
package eu.allodia.mailcal

import uniffi.mailcal_bindings.MissReason
import uniffi.mailcal_bindings.SetupRecommendation

// The flow's phases: type an email, wait, then either a routed card or the manual form.
internal sealed interface DetectPhase {
    object Email : DetectPhase
    object Detecting : DetectPhase

    // `signInOffered` is resolved *before* this phase is entered, so the card renders in its
    // final shape. Resolving it afterwards meant the secret field appeared for a second and then
    // vanished under the user, the flash this design exists to prevent.
    data class Found(val recommendation: SetupRecommendation, val signInOffered: Boolean) :
        DetectPhase

    data class Manual(val reason: MissReason?, val edit: SetupRecommendation?) : DetectPhase
}

// Routes a detection result: a Manual result drops to the manual form with its reason;
// everything else shows a routed card.
internal fun route(recommendation: SetupRecommendation, signInOffered: Boolean): DetectPhase =
    when (recommendation) {
        is SetupRecommendation.Manual ->
            DetectPhase.Manual(reason = recommendation.reason, edit = null)
        else -> DetectPhase.Found(recommendation, signInOffered)
    }

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
)

internal fun manualPrefill(edit: SetupRecommendation?): ManualPrefill = when (edit) {
    is SetupRecommendation.Imap ->
        ManualPrefill(AccountKind.PASSWORD, edit.email, edit.imapHost, edit.smtpHost ?: "", "")
    is SetupRecommendation.Jmap ->
        ManualPrefill(AccountKind.JMAP, edit.email, jmapServer = edit.serverUrl)
    is SetupRecommendation.Microsoft -> ManualPrefill(AccountKind.MICROSOFT, edit.email)
    is SetupRecommendation.Google -> ManualPrefill(AccountKind.GOOGLE, edit.email)
    else -> ManualPrefill()
}
