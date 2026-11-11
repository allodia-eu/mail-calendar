// The redirect an **Allodia account** sign-in returns to, and the service's own pages. The Rust
// core runs the sign-in from the press to the stored grant, and the browser sign-in driver opens it
// and hands back the custom-scheme redirect the AndroidManifest intent-filter routes to
// MainActivity.onNewIntent (MainActivityBrowserSignIn.kt).
//
// An Allodia account is not a mail account: it carries no mailbox, appears in no switcher, and a
// token issued for it cannot touch anyone's mail. Its screen is Settings → Allodia account.
package eu.allodia.mailcal

import android.content.Context
import android.net.Uri
import androidx.browser.customtabs.CustomTabsIntent

// The redirect the account service sends the browser back to. The client registration is injected
// into the core at build time and never appears here; what the host needs is the URI it must catch,
// which is registered statically against this application id (see the entitlement contract that
// ships beside the Allodia Licence).
internal object AllodiaOAuthConfig {
    // The redirect HOST that identifies an Allodia callback. It rides the same application-id
    // scheme the JMAP flow uses, so the host is the only thing that tells the two apart, see
    // OAuthRedirect. Deliberately not `auth` or `jmap-oauth` for exactly that reason: two flows
    // sharing a label is a redirect delivered to the wrong one, which fails by never coming back
    // rather than by erroring.
    const val REDIRECT_HOST = "account-oauth"

    // The full redirect URI sent with `AllodiaIntent.SignIn`, and registered with the service. A
    // mismatch is rejected as redirect_uri_mismatch, so this must equal the manifest filter's
    // scheme + host character for character.
    val REDIRECT_URI = "${BuildConfig.APPLICATION_ID}://$REDIRECT_HOST"
}

// Opens the service's own account page, or the page that ends its session, in a Custom Tab.
//
// A Custom Tab IS the browser, so it carries the session cookie the sign-in set and the page
// opens already signed in. A WebView has its own cookie jar and would show a login form instead,
// and Google refuses an embedded user-agent for the sign-in that form offers.
internal fun openAllodiaAccountPage(context: Context, accountUrl: String) {
    CustomTabsIntent.Builder().build().launchUrl(context, Uri.parse(accountUrl))
}
