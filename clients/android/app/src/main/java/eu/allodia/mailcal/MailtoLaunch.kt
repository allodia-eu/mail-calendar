// Deciding whether an incoming Intent is a mail link. Kept as a plain object, apart from the
// Activity, so the JVM suite can gate it directly (see MailtoLaunchTest), the *parsing* of the
// URI is the shared core's job and is covered by its own Rust tests, so nothing here loads the
// cdylib.
package eu.allodia.mailcal

import android.content.Intent

internal object MailtoLaunch {

    // The two actions that carry a mail link: ACTION_VIEW for a tapped `mailto:` link, and
    // ACTION_SENDTO for another app (or the system chooser) asking for a mail client. Both are
    // declared in the manifest, so both arrive here.
    private val MAIL_LINK_ACTIONS = setOf(Intent.ACTION_VIEW, Intent.ACTION_SENDTO)

    // Whether this launch is a mail link we should open the composer for.
    //
    // The scheme check is what keeps this off the OAuth redirects: those arrive as ACTION_VIEW
    // too (msauth:, com.googleusercontent.apps.*, eu.allodia.mailcal:), and treating one as a
    // mail link would swallow a sign-in and pop a composer in the middle of adding an account.
    // Matching on the action alone is the bug this function exists to prevent.
    //
    // Whether the URI is a *well-formed* mailto is deliberately not decided here, the shared
    // core answers that (and drops the headers a link may not set), so every platform agrees.
    fun carriesMailLink(action: String?, scheme: String?): Boolean =
        action in MAIL_LINK_ACTIONS && scheme.equals("mailto", ignoreCase = true)
}
