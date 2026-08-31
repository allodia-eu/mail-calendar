package eu.allodia.mailcal

import android.content.Context
import android.content.Intent
import android.net.Uri
import androidx.test.core.app.ApplicationProvider
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

/**
 * The app's identity is injected at build time (docs/branding.md), which means it now arrives by
 * two routes at once, the manifest, from Gradle, and the catalog, from the l10n codegen, and a
 * build where those disagree still compiles, installs and runs. These are the assertions that
 * disagreement fails.
 *
 * They pin relationships rather than values: an unbranded build is as correct as a branded one, so
 * asserting "Allodia Mail & Calendar" here would only assert which checkout the test ran in.
 */
@RunWith(RobolectricTestRunner::class)
class BrandIdentityTest {
    private val context: Context = ApplicationProvider.getApplicationContext()

    @Test
    fun the_launcher_label_is_the_name_the_app_calls_itself() {
        // Two different files, one name: `android:label` comes from the Gradle placeholder and
        // `app_title` from the substituted catalog. A user who reads one and then the other must
        // not see two products.
        val label = context.applicationInfo.loadLabel(context.packageManager).toString()

        assertEquals(L10n.app_title(context), label)
    }

    @Test
    fun the_jmap_redirect_comes_back_to_this_app() {
        // The core sends this exact URI to the server as the client's redirect_uri when it
        // registers (RFC 7591). If the manifest filter and the constant have drifted, the server
        // accepts the sign-in and the browser then has nowhere to deliver it to.
        val delivered = context.packageManager.queryIntentActivities(
            Intent(Intent.ACTION_VIEW, Uri.parse(JmapOAuthConfig.REDIRECT_URI))
                .addCategory(Intent.CATEGORY_BROWSABLE),
            0,
        )

        assertTrue(
            "no activity accepts ${JmapOAuthConfig.REDIRECT_URI}",
            delivered.any { it.activityInfo.name == MainActivity::class.java.name },
        )
    }

    @Test
    fun the_microsoft_redirect_is_addressed_to_this_app() {
        // MSAL's format is msauth://<package>/<signature-hash>, and Azure registers it against
        // this app's package name, so the host is not free-form text that happens to look like
        // the id.
        val host = Uri.parse(MicrosoftOAuthConfig.REDIRECT_URI).host

        assertEquals(context.packageName, host)
    }
}
