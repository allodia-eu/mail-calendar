// What a mail account's setup card asks for, once its server has answered.
//
// Three states rather than a flag (docs/mail-oauth.md rule 2), and the middle one is why: a
// provider whose sign-in exists but admits only applications it registered in advance is not the
// same as one that offers none, and showing one bare password form for both leaves somebody
// wondering why the button their colleague has is missing.
//
// Compose-free, so the JVM suite drives it without composing anything.
package eu.allodia.mailcal

import uniffi.mailcal_bindings.ConnectionSecurity
import uniffi.mailcal_bindings.ImapAuthOffer
import uniffi.mailcal_bindings.ImapLoginRequest
import uniffi.mailcal_bindings.SetupRecommendation

internal sealed interface ImapAuthState {
    // Sign in with the provider. [passwordAlsoWorks] decides whether the password field sits
    // below it, or whether that route would be a dead end.
    data class SignIn(val passwordAlsoWorks: Boolean) : ImapAuthState

    // The provider's sign-in exists but is closed to this application.
    object RegistrationNeeded : ImapAuthState

    // No sign-in here: the password form, as it always was. Also what a failure, a missing core
    // and a silent server all come to.
    object Password : ImapAuthState

    // A sign-in was started and did not finish. The password field comes back, because it is the
    // route left, and the reason is said rather than left to be guessed at.
    object Failed : ImapAuthState

    // Whether the sign-in button belongs on screen.
    val offersSignIn: Boolean get() = this is SignIn

    // Whether to explain that this provider admits only pre-registered applications.
    val explainsRegistration: Boolean get() = this is RegistrationNeeded

    // Whether the password field belongs on screen.
    //
    // Not where the server said a password is refused: on a provider that has switched password
    // authentication off, that field is a dead end nobody finds until they have typed one.
    val showsPassword: Boolean get() = when (this) {
        is SignIn -> passwordAlsoWorks
        RegistrationNeeded, Password, Failed -> true
    }

    companion object {
        fun of(offer: ImapAuthOffer): ImapAuthState = when (offer) {
            is ImapAuthOffer.SignIn -> SignIn(offer.passwordAlsoWorks)
            is ImapAuthOffer.RegistrationNeeded -> RegistrationNeeded
            is ImapAuthOffer.Password -> Password
        }
    }
}

// The account a pre-flight and a sign-in both describe.
//
// One builder used by both, so the two cannot come to different conclusions about the same
// account: a pre-flight that probed a different server from the one the sign-in registers against
// would offer a button that fails at the provider.
internal fun imapLoginRequest(
    recommendation: SetupRecommendation.Imap,
    caldavBaseUrl: String? = null,
): ImapLoginRequest = ImapLoginRequest(
    email = recommendation.email,
    imapHost = recommendation.imapHost,
    smtpHost = recommendation.smtpHost,
    caldavBaseUrl = caldavBaseUrl,
    imapSecurity = recommendation.imapSecurity,
    smtpSecurity = recommendation.smtpSecurity,
    oauthIssuer = recommendation.oauthIssuer,
)

// The same, for the manual form's typed fields. Nothing was detected, so no provider named an
// issuer for itself and the core's well-known probe is what answers.
internal fun typedImapLoginRequest(
    email: String,
    imapHost: String,
    smtpHost: String,
    caldavUrl: String,
): ImapLoginRequest = ImapLoginRequest(
    email = email,
    imapHost = imapHost,
    smtpHost = smtpHost.ifBlank { null },
    caldavBaseUrl = caldavUrl.ifBlank { null },
    // The manual form is implicit-TLS only; a STARTTLS server arrives through autodetection
    // (docs/account-autodetect.md → Known gaps).
    imapSecurity = ConnectionSecurity.IMPLICIT_TLS,
    smtpSecurity = ConnectionSecurity.IMPLICIT_TLS,
    oauthIssuer = null,
)
