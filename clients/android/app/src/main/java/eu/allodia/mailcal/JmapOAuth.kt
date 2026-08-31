// Config + browser launch for the JMAP "sign in with your provider" flow on Android. Sibling of
// MicrosoftOAuth.kt / GoogleOAuth.kt, and deliberately much smaller than either: there is no
// client id to embed here, because a JMAP server is not a provider we integrated at build time.
// The Rust core discovers the authorization server from the standards (RFC 9728 → 8414) and
// registers this install as a client on the fly (RFC 7591), so all this host owns is opening the
// authorization URL and catching the redirect.
package eu.allodia.mailcal

import android.content.Context
import android.net.Uri
import androidx.browser.customtabs.CustomTabsIntent

internal object JmapOAuthConfig {
    // Our own app scheme, so the redirect is unambiguous no matter which server the user is
    // signing in to (unlike Microsoft/Google we cannot key on a provider-issued scheme, there is
    // no provider registration). MUST equal the android:scheme + android:host of the JMAP
    // intent-filter in AndroidManifest.xml, character for character, and is sent to the server at
    // registration time as the client's redirect_uri.
    //
    // Both sides therefore read the application id rather than repeating it: the filter through
    // the manifest's applicationId placeholder, this through BuildConfig, so a re-branded build
    // (docs/branding.md) moves them together. The id is lowercase by rule, which this relies on:
    // Android compares an intent filter's scheme case-sensitively and never matches an uppercase
    // one.
    val REDIRECT_SCHEME = BuildConfig.APPLICATION_ID
    val REDIRECT_URI = "$REDIRECT_SCHEME://jmap-oauth"
}

// Opens [authorizationUrl] in a Custom Tab (the user's browser, with its existing session). The
// redirect back to REDIRECT_URI is delivered to MainActivity via the manifest intent-filter +
// onNewIntent.
//
// The tab is launched into its **own task** (FLAG_ACTIVITY_NEW_TASK). Without that it sits on top
// of MainActivity's task, and MainActivity is `launchMode="singleTask"`, so the moment the user
// leaves for their password manager and comes back through the launcher, Android brings
// MainActivity to the front and finishes everything above it, silently destroying the half-
// finished sign-in. Reaching for a password is not an exotic thing to do during a sign-in; it is
// the single most likely thing to do.
internal fun openJmapSignIn(context: Context, authorizationUrl: String) {
    val intent = CustomTabsIntent.Builder().build()
    intent.intent.addFlags(android.content.Intent.FLAG_ACTIVITY_NEW_TASK)
    intent.launchUrl(context, Uri.parse(authorizationUrl))
}
