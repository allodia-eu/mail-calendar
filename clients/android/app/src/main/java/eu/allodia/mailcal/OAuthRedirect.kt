// Which sign-in an arriving custom-scheme redirect belongs to.
//
// Five browser flows come back to this activity, and three of them, IMAP, JMAP and the Allodia
// account, ride the SAME scheme: the application id. Only the redirect's host tells those apart. A
// dispatch on the scheme alone hands one flow's redirect to another, which does not error: the
// exchange is handed a code minted for a different client and the sign-in the person is actually
// waiting on never comes back.
//
// Kept a pure function of what the intent carried so the JVM suite can assert it: the schemes the
// two provider flows watch for are properties of the injected build (`oauthRoutes()`), which the
// tests cannot call, so they are passed in.
package eu.allodia.mailcal

internal enum class OAuthRedirect {
    GOOGLE,
    MICROSOFT,
    ALLODIA,
    IMAP,
    JMAP,
    ;

    companion object {
        /**
         * The flow [scheme] + [host] belong to, or null when this is not one of our redirects.
         *
         * [googleScheme] and [microsoftScheme] are null in a build carrying no registration for
         * that provider; a schemeless intent must not match one of them by both being null, which
         * is why the caller's scheme is required non-null.
         */
        fun of(
            scheme: String?,
            host: String?,
            googleScheme: String?,
            microsoftScheme: String?,
            appScheme: String,
        ): OAuthRedirect? {
            if (scheme == null) return null
            return when (scheme) {
                googleScheme -> GOOGLE
                microsoftScheme -> MICROSOFT
                appScheme -> when (host) {
                    AllodiaOAuthConfig.REDIRECT_HOST -> ALLODIA
                    ImapOAuthConfig.REDIRECT_HOST -> IMAP
                    else -> JMAP
                }
                else -> null
            }
        }
    }
}
