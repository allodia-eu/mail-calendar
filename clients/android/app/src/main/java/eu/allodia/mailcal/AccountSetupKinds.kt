// The account types the manual setup form offers, and the picker that chooses one. A menu rather
// than a row of segments: five types, one of them "Calendar and contacts (CalDAV/CardDAV)", do not
// fit across a phone. Split from AccountSetupScreen.kt to keep that file under the 500-line limit.
package eu.allodia.mailcal

import android.content.Context
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.painterResource
import uniffi.mailcal_bindings.OAuthRoutes

// The account kinds the setup form can offer (the two OAuth providers, Microsoft and Google, sit
// together at the end); which of them this build shows is [AccountKind.offered].
internal enum class AccountKind {
    PASSWORD,
    JMAP,
    // A calendar and contacts server without mail (docs/accounts.md).
    DAV,
    MICROSOFT,
    GOOGLE,
    ;

    internal fun label(ctx: Context): String = when (this) {
        PASSWORD -> L10n.setup_account_type_password(ctx)
        JMAP -> L10n.setup_account_type_jmap(ctx)
        DAV -> L10n.setup_account_type_dav(ctx)
        MICROSOFT -> L10n.setup_account_type_microsoft(ctx)
        GOOGLE -> L10n.setup_account_type_google(ctx)
    }

    internal companion object {
        // The kinds a build offering [routes] shows, in picker order. A browser sign-in needs the
        // provider's OAuth client registration, which is injected at build time, so a build given
        // none drops the route rather than showing a button that fails at the provider. The two
        // credential routes are always there.
        //
        // Takes the routes rather than asking the core for them, so this is a pure function the
        // JVM suite can exercise, asking would load the cdylib, which no test here does.
        internal fun offered(routes: OAuthRoutes): List<AccountKind> = entries.filter { kind ->
            when (kind) {
                MICROSOFT -> routes.microsoft
                GOOGLE -> routes.google
                PASSWORD, JMAP, DAV -> true
            }
        }
    }
}

// The account type, as a menu under its label.
@Composable
internal fun AccountKindPicker(offered: List<AccountKind>, kind: AccountKind, onPick: (AccountKind) -> Unit) {
    val ctx = LocalContext.current
    var expanded by remember { mutableStateOf(false) }
    Column {
        Text(L10n.setup_account_type(ctx), style = MaterialTheme.typography.labelLarge)
        Box {
            OutlinedButton(
                onClick = { expanded = true },
                modifier = Modifier.fillMaxWidth().testTag("setup-kind"),
            ) {
                Text(kind.label(ctx), modifier = Modifier.weight(1f))
                Icon(painter = painterResource(R.drawable.ic_arrow_drop_down), contentDescription = null)
            }
            DropdownMenu(expanded = expanded, onDismissRequest = { expanded = false }) {
                offered.forEach { option ->
                    DropdownMenuItem(
                        text = { Text(option.label(ctx)) },
                        onClick = {
                            expanded = false
                            onPick(option)
                        },
                    )
                }
            }
        }
    }
}
