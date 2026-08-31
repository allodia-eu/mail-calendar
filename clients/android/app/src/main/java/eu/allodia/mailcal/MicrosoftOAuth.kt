// Config + browser launch for the Microsoft 365 OAuth sign-in on Android. The Rust core owns
// the OAuth state machine (PKCE, token exchange, refresh); this host owns only opening the
// authorization URL in the user's browser (Chrome Custom Tabs, reusing its logged-in Microsoft
// session) and catching the custom-scheme redirect, which the AndroidManifest intent-filter
// routes to MainActivity.onNewIntent.
package eu.allodia.mailcal

import android.content.Context
import android.net.Uri
import androidx.browser.customtabs.CustomTabsIntent

// The Android half of the Azure app registration. The client id is injected into the core at
// build time and never appears here; the redirect cannot be, because Azure registers it against
// this app's package name and signing certificate. REDIRECT_URI must match a redirect registered
// under "Mobile and desktop applications" in the Azure portal AND the scheme/host in the
// AndroidManifest intent-filter, character for character. There is deliberately no client secret
// on the device: PKCE, owned by the core, stands in for one.
internal object MicrosoftOAuthConfig {
    const val TENANT = "common"
    // The Android (MSAL-format) redirect URI: msauth://<package>/<url-encoded-signature-hash>.
    // Using it (rather than a plain custom scheme) gives a consistent experience with Microsoft
    // Authenticator on Android. Must match the redirect registered in the Azure app AND the
    // scheme/host/path in the AndroidManifest intent-filter (which uses the *decoded* hash).
    //
    // The host is the application id, read from BuildConfig here and from the applicationId
    // placeholder in the manifest, so the two cannot drift when the app is re-branded
    // (docs/branding.md). A build carrying a different id is one Azure has no registration for:
    // which is consistent, because such a build has no Microsoft client id either and never
    // offers the route.
    val REDIRECT_URI = "msauth://${BuildConfig.APPLICATION_ID}/VzSiQcXRmi2kyjzcA%2BmYLEtbGVs%3D"
    // The scheme of REDIRECT_URI, the manifest intent-filter (scheme+host+path) already scopes
    // the redirect to us, so onNewIntent just checks the scheme.
    const val REDIRECT_SCHEME = "msauth"
}

// Opens [authorizationUrl] in a Custom Tab (the user's browser). The redirect back to the
// custom scheme is delivered to MainActivity via the manifest intent-filter + onNewIntent.
internal fun openMicrosoftSignIn(context: Context, authorizationUrl: String) {
    CustomTabsIntent.Builder().build().launchUrl(context, Uri.parse(authorizationUrl))
}
