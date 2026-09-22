// The Compose-free half of the email-first setup flow, split out of AccountSetupDetect.kt so the
// JVM suite can drive it without composing anything: the untrusted-settings approval gate in
// particular is a security contract worth covering.
package eu.allodia.mailcal

import uniffi.mailcal_bindings.AccountSetup
import uniffi.mailcal_bindings.JmapSetup
import uniffi.mailcal_bindings.RejectedCertificate
import uniffi.mailcal_bindings.SetupRecommendation

// What a connect that did not succeed has to say. A refused certificate is its own outcome
// rather than a message, because it is the one failure the form can offer a way past
// (docs/certificate-exceptions.md).
internal data class ConnectFailure(
    val message: String,
    val certificate: RejectedCertificate? = null,
)

// Tracks the user's input against a JMAP/IMAP detection result and decides whether Connect is
// allowed and what payload to submit. Pure and Compose-free so it can be unit-tested: the
// untrusted-settings approval gate in particular is a security contract worth covering.
internal class DetectedConnectForm(val recommendation: SetupRecommendation) {
    // One secret for both routes: IMAP takes a password, and a JMAP server declares its auth
    // scheme in its own 401 challenge, so a password and an API token are interchangeable here.
    var password: String = ""
    var approved: Boolean = false

    // The certificate the last connect was refused for, when it was refused for one. Until it
    // is accepted, Connect stays inert: the same gate the untrusted-settings approval uses, for
    // the same reason (docs/certificate-exceptions.md).
    var rejectedCertificate: RejectedCertificate? = null
    var certificateAccepted: Boolean = false

    // Calendar (IMAP only). When detection found a CalDAV endpoint we pre-select sync (opt-out);
    // otherwise the user can opt in and type one. Either way it reuses the IMAP credentials.
    private val detectedCaldav: String? = (recommendation as? SetupRecommendation.Imap)?.caldavUrl
    var calendarEnabled: Boolean = detectedCaldav != null
    var calendarUrlEntry: String = ""

    // The CalDAV URL to store: the discovered endpoint, else a manually entered one; null when
    // calendar is switched off or nothing was entered.
    val effectiveCaldavUrl: String?
        get() = if (!calendarEnabled) null else detectedCaldav ?: calendarUrlEntry.ifBlank { null }

    val isTrusted: Boolean = when (recommendation) {
        is SetupRecommendation.Jmap -> recommendation.isTrusted
        is SetupRecommendation.Imap -> recommendation.isTrusted
        else -> true
    }

    // Untrusted settings (a non-HTTPS hop, e.g. an http autoconfig) must be explicitly approved.
    val needsApproval: Boolean get() = !isTrusted
    private val approvalOk: Boolean get() = isTrusted || approved
    // The refused certificate this route can actually do something about. Only an IMAP
    // account's stored config carries an exception, so a JMAP refusal is reported and not
    // offered: taking an answer and ignoring it is worse than not asking
    // (docs/certificate-exceptions.md -> Known gaps).
    val refusedCertificate: RejectedCertificate?
        get() = rejectedCertificate.takeIf { recommendation is SetupRecommendation.Imap }

    private val certificateOk: Boolean get() = refusedCertificate == null || certificateAccepted

    // The certificate to store with the account: the refused one, once accepted, and nothing
    // at all otherwise.
    val acceptedCertificate: RejectedCertificate?
        get() = if (certificateAccepted) refusedCertificate else null

    val canConnect: Boolean get() = when (recommendation) {
        is SetupRecommendation.Jmap -> password.isNotBlank() && approvalOk && certificateOk
        is SetupRecommendation.Imap -> password.isNotBlank() && approvalOk && certificateOk
        else -> false
    }

    fun jmapSetup(): JmapSetup {
        val jmap = recommendation as SetupRecommendation.Jmap
        return JmapSetup(
            email = jmap.email,
            serverUrl = jmap.serverUrl.ifBlank { null },
            password = password,
        )
    }

    fun imapSetup(): AccountSetup {
        val imap = recommendation as SetupRecommendation.Imap
        return AccountSetup(
            imapHost = imap.imapHost,
            username = imap.email,
            password = password,
            smtpHost = imap.smtpHost,
            caldavBaseUrl = effectiveCaldavUrl,
            // Carry the detected connection security straight through so the engine dials
            // implicit TLS or STARTTLS exactly as detection found it.
            imapSecurity = imap.imapSecurity,
            smtpSecurity = imap.smtpSecurity,
            // Set only on a re-submit somebody asked for after being shown the certificate; it
            // is stored with the account, so no later connect asks again.
            acceptedCertificate = acceptedCertificate,
        )
    }
}

