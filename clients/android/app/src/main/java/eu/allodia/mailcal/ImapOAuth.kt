// Config + browser launch for the IMAP "sign in with your provider" flow. Sibling of JmapOAuth.kt,
// and for the same reason there is no client id here: the server was discovered at runtime, not
// integrated at build time, so the Rust core registers this install with it on the fly (RFC 7591)
// and this host owns only the browser hop.
//
// What a mail account's setup screen asks for before any of this runs, and why there are three
// answers rather than two, is docs/mail-oauth.md.
package eu.allodia.mailcal

import android.content.Context
import android.net.Uri
import androidx.browser.customtabs.CustomTabsIntent

internal object ImapOAuthConfig {
    // Its own host under the application-id scheme, beside the JMAP and Allodia ones. Two flows
    // sharing a host would be told apart by nothing, and a redirect handed to the wrong flow does
    // not error: it is exchanged against a different client and the sign-in the person is waiting
    // on simply never comes back.
    //
    // MUST equal the android:scheme + android:host of the IMAP intent-filter in
    // AndroidManifest.xml character for character, and is what the core sends as the client's
    // `redirect_uri` at registration time.
    const val REDIRECT_HOST = "imap-oauth"
    val REDIRECT_URI = "${JmapOAuthConfig.REDIRECT_SCHEME}://$REDIRECT_HOST"
}

// Opens [authorizationUrl] in a Custom Tab (the person's browser, with its existing session). The
// redirect back to REDIRECT_URI reaches MainActivity via the manifest intent-filter + onNewIntent.
//
// Launched into its **own task** for the reason JmapOAuth.kt records at length: MainActivity is
// `singleTask`, so a tab sitting on its stack is destroyed the moment somebody leaves for their
// password manager and comes back through the launcher. Reaching for a password manager during a
// sign-in is not an exotic thing to do; it is the most likely thing to do.
internal fun openImapSignIn(context: Context, authorizationUrl: String) {
    val intent = CustomTabsIntent.Builder().build()
    intent.intent.addFlags(android.content.Intent.FLAG_ACTIVITY_NEW_TASK)
    intent.launchUrl(context, Uri.parse(authorizationUrl))
}
