// Config + browser launch for the Google (Gmail + Google Calendar) OAuth sign-in on Android. The
// Rust core owns the OAuth state machine (PKCE, token exchange, refresh); this host owns only
// opening the authorization URL in the user's browser (Chrome Custom Tabs, reusing its logged-in
// Google session) and catching the custom-scheme redirect, which the AndroidManifest intent-filter
// routes to MainActivity.onNewIntent. Sibling of MicrosoftOAuth.kt.
package eu.allodia.mailcal

import android.content.Context
import android.net.Uri
import androidx.browser.customtabs.CustomTabsIntent
import uniffi.mailcal_bindings.oauthRoutes

// The Android half of the Google OAuth client. The client id itself is injected into the core at
// build time and never appears here; what the host needs is the redirect it must catch, which the
// core derives from that id, the "reversed client id" custom scheme
// com.googleusercontent.apps.<CLIENT_ID_SUFFIX>:/oauth2redirect. There is deliberately no client
// secret on the device: an Android client has none, and PKCE (owned by the core) is the protection.
//
// Everything here is null when the build carries no Google registration, in which case the setup
// screen never offers the route. The build also feeds the same client id to the
// googleRedirectScheme manifest placeholder, so the intent-filter and this agree by construction:
// and a build without one registers a scheme no browser will ever redirect to.
internal object GoogleOAuthConfig {
    // The reversed-client-id custom scheme: the *whole* client id (everything before
    // ".apps.googleusercontent.com", INCLUDING the numeric project-number prefix) with its dotted
    // components reversed. Google matches an Android redirect solely by this scheme; drop the
    // project-number prefix and it fails with redirect_uri_mismatch.
    val REDIRECT_SCHEME: String?
        get() = REDIRECT_URI?.substringBefore(':')

    // The Android redirect URI: the reversed-client-id scheme plus the /oauth2redirect path.
    val REDIRECT_URI: String?
        get() = oauthRoutes().googleRedirectUri
}

// Opens [authorizationUrl] in a Custom Tab (the user's browser). The redirect back to the
// reversed-client-id scheme is delivered to MainActivity via the manifest intent-filter +
// onNewIntent.
internal fun openGoogleSignIn(context: Context, authorizationUrl: String) {
    CustomTabsIntent.Builder().build().launchUrl(context, Uri.parse(authorizationUrl))
}
