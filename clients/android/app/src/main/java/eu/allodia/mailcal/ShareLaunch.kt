// Deciding whether an incoming Intent is a share, and pulling the payload out of the two shapes
// Android delivers one in. Kept as a plain object, apart from the Activity, so the JVM suite can
// gate it directly (see ShareLaunchTest); what the payload *means* is the shared core's job
// (prefillFromShare), so nothing here loads the cdylib, names a file, or decides a media type.
//
// The product contract is docs/os-integration.md; the security half is Gate 13 in
// docs/composer-security.md.
package eu.allodia.mailcal

import android.content.Intent
import android.net.Uri
import android.os.Build

internal object ShareLaunch {

    // The two actions that carry a share. ACTION_SEND is one item, ACTION_SEND_MULTIPLE several;
    // both are declared in the manifest, so both arrive here.
    private val SHARE_ACTIONS = setOf(Intent.ACTION_SEND, Intent.ACTION_SEND_MULTIPLE)

    // Whether this launch is a share we should open a composer for.
    //
    // Deliberately NOT the mirror of MailtoLaunch.carriesMailLink: that one has to gate on the
    // scheme because the OAuth redirects arrive as ACTION_VIEW too. Nothing else in this app is
    // dispatched on ACTION_SEND, so the action alone settles it, and demanding a mimeType here
    // would only re-check what the manifest filter already decided.
    fun carriesShare(action: String?): Boolean = action in SHARE_ACTIONS

    // The content the share carries, in the order the sending app supplied it.
    //
    // ACTION_SEND puts one item in EXTRA_STREAM and ACTION_SEND_MULTIPLE a list, and a sender may
    // legitimately use either extra with either action, so both are read rather than switching on
    // the action: a share that named its item the "wrong" way is still the user asking us to send
    // that file.
    fun sharedUris(intent: Intent): List<Uri> {
        val single = listOfNotNull(intent.parcelable(Intent.EXTRA_STREAM, Uri::class.java))
        val many = intent.parcelableList(Intent.EXTRA_STREAM, Uri::class.java)
        return (single + many).distinct()
    }

    // Text the share carried: a selection, a URL, or a whole `mailto:` link. Blank when it carried
    // none. The core decides which of those it is; this only hands it over.
    fun sharedText(intent: Intent): String =
        intent.getCharSequenceExtra(Intent.EXTRA_TEXT)?.toString().orEmpty()

    // A subject the sending app suggested (a browser shares the page title this way). Blank when
    // absent.
    fun sharedSubject(intent: Intent): String =
        intent.getCharSequenceExtra(Intent.EXTRA_SUBJECT)?.toString().orEmpty()

    // `getParcelableExtra` is deprecated below the typed overload added in Tiramisu. minSdk is 31,
    // so both branches are reachable and the untyped call is the only way to read one on 31/32.
    private fun <T> Intent.parcelable(name: String, type: Class<T>): T? =
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            getParcelableExtra(name, type)
        } else {
            @Suppress("DEPRECATION")
            getParcelableExtra(name)
        }

    private fun <T> Intent.parcelableList(name: String, type: Class<T>): List<T> =
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            getParcelableArrayListExtra(name, type).orEmpty()
        } else {
            @Suppress("DEPRECATION")
            getParcelableArrayListExtra<android.os.Parcelable>(name)
                .orEmpty()
                .filterIsInstance(type)
        }
}
