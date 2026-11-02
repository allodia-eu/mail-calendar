// Which way a Gmail account's mail syncs in this launch, on its page (docs/settings.md row 9):
// Optimised, Fallback, or Checking until the first read has settled it. The status never names a
// protocol; the info button opens what it means and a link to the help page.
package eu.allodia.mailcal

import android.content.Context
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.Card
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalUriHandler
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.unit.dp
import uniffi.mailcal_bindings.HelpPage
import uniffi.mailcal_bindings.MailSyncTechnology
import uniffi.mailcal_bindings.helpPageUrl

/** What each sync technology is called and explained as, apart from the view so it can be tested. */
internal object AccountSyncTechnologyLabels {
    fun name(ctx: Context, technology: MailSyncTechnology): String = when (technology) {
        MailSyncTechnology.CHECKING -> L10n.settings_account_sync_technology_checking(ctx)
        MailSyncTechnology.OPTIMISED -> L10n.settings_account_sync_technology_optimised(ctx)
        MailSyncTechnology.FALLBACK -> L10n.settings_account_sync_technology_fallback(ctx)
    }

    fun detail(ctx: Context, technology: MailSyncTechnology): String = when (technology) {
        MailSyncTechnology.CHECKING -> L10n.settings_account_sync_technology_checking_detail(ctx)
        MailSyncTechnology.OPTIMISED -> L10n.settings_account_sync_technology_optimised_detail(ctx)
        MailSyncTechnology.FALLBACK -> L10n.settings_account_sync_technology_fallback_detail(ctx)
    }

    /** The help page in the language the app is shown in, where the help site has it. */
    fun helpUrl(ctx: Context): String =
        helpPageUrl(HelpPage.OPTIMISE_GMAIL_SYNC, ctx.resources.configuration.locales[0].language)
}

@Composable
internal fun AccountSyncTechnologyCard(technology: MailSyncTechnology) {
    val ctx = LocalContext.current
    val uriHandler = LocalUriHandler.current
    var explaining by remember { mutableStateOf(false) }
    val dot = when (technology) {
        MailSyncTechnology.CHECKING -> MaterialTheme.colorScheme.outline
        MailSyncTechnology.OPTIMISED -> Color(0xFF2E7D32)
        MailSyncTechnology.FALLBACK -> Color(0xFFEF6C00)
    }
    Card(modifier = Modifier.fillMaxWidth()) {
        Row(
            modifier = Modifier.fillMaxWidth().padding(start = 16.dp, end = 4.dp, top = 4.dp, bottom = 4.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Text(
                L10n.settings_account_sync_technology_heading(ctx),
                style = MaterialTheme.typography.titleMedium,
                modifier = Modifier.weight(1f),
            )
            Box(
                modifier = Modifier.size(8.dp).clip(CircleShape).background(dot)
                    .clearAndSetSemantics {},
            )
            Spacer(modifier = Modifier.size(6.dp))
            Text(
                AccountSyncTechnologyLabels.name(ctx, technology),
                modifier = Modifier.testTag("account-sync-technology"),
            )
            Box {
                IconButton(onClick = { explaining = true }) {
                    Icon(
                        painter = painterResource(R.drawable.ic_info),
                        contentDescription = L10n.settings_account_sync_technology_about(ctx),
                    )
                }
                DropdownMenu(expanded = explaining, onDismissRequest = { explaining = false }) {
                    Column(
                        modifier = Modifier.widthIn(max = 280.dp).padding(horizontal = 16.dp),
                        verticalArrangement = Arrangement.spacedBy(4.dp),
                    ) {
                        Text(AccountSyncTechnologyLabels.detail(ctx, technology))
                        TextButton(onClick = {
                            explaining = false
                            uriHandler.openUri(AccountSyncTechnologyLabels.helpUrl(ctx))
                        }) {
                            Text(L10n.settings_account_sync_technology_learn_more(ctx))
                        }
                    }
                }
            }
        }
    }
}
