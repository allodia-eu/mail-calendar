// Settings → About (Android): which release this is, where to ask for help, and whose work it is
// built on. Its own file so SettingsScreen.kt stays under the 500-line limit, the same way
// SettingsNotifications.kt is split out. The content is the core's (aboutInfo) so every client
// says the same thing; only the labels around it come from the catalog.
//
// The payload is passed in rather than fetched here: `aboutInfo` is a call into the cdylib, and
// the JVM suite never loads it (AGENTS.md), so a composable that called it could not be rendered
// in a test at all. MainActivity reads it once, where every other snapshot is read.
package eu.allodia.mailcal

import android.content.Context
import android.content.Intent
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import androidx.core.net.toUri
import uniffi.mailcal_bindings.AboutInfo

@Composable
internal fun AboutSection(about: AboutInfo) {
    val ctx = LocalContext.current

    SettingsGroupCard(L10n.app_title(ctx), L10n.about_version(ctx, about.version)) {}
    Spacer(modifier = Modifier.height(12.dp))

    SettingsGroupCard(L10n.about_support_heading(ctx), L10n.about_support_description(ctx)) {
        Text(about.supportUrl, style = MaterialTheme.typography.bodyMedium)
        TextButton(onClick = { openSupport(ctx, about.supportUrl) }) {
            Text(L10n.about_support_action(ctx))
        }
    }
    Spacer(modifier = Modifier.height(12.dp))

    SettingsGroupCard(
        L10n.about_attributions_heading(ctx),
        L10n.about_attributions_description(ctx),
    ) {
        Column(modifier = Modifier.fillMaxWidth()) {
            about.attributions.forEach { item ->
                Text(item.name, style = MaterialTheme.typography.bodyMedium)
                Text(
                    item.license,
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                Spacer(modifier = Modifier.height(8.dp))
            }
        }
    }
}

// The forum opens in the user's browser, never in an in-app WebView: this app's WebViews are the
// locked-down reading and composing islands (docs/rendering-security.md), and neither is a browser.
private fun openSupport(ctx: Context, url: String) {
    ctx.startActivity(Intent(Intent.ACTION_VIEW, url.toUri()))
}
