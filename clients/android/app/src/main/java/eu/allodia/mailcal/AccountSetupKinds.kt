// The account types the manual setup form offers, and the picker that chooses one. A menu rather
// than a row of segments: six types, one of them "Calendar and contacts (CalDAV/CardDAV)", do not
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
import uniffi.mailcal_bindings.ManualSetupKind

// The account kinds the setup form can offer; which of them this build shows, and in which order,
// is the core's (`manual_setup_kinds`), read through [AccountKind.of].
internal enum class AccountKind {
    PASSWORD,
    JMAP,
    MICROSOFT,
    GOOGLE,
    // A Nextcloud server's calendar and contacts, signed in to from its address alone
    // (docs/nextcloud-sign-in.md rule 2).
    NEXTCLOUD,
    // A calendar and contacts server without mail (docs/accounts.md).
    DAV,
    ;

    internal fun label(ctx: Context): String = when (this) {
        PASSWORD -> L10n.setup_account_type_password(ctx)
        JMAP -> L10n.setup_account_type_jmap(ctx)
        DAV -> L10n.setup_account_type_dav(ctx)
        MICROSOFT -> L10n.setup_account_type_microsoft(ctx)
        GOOGLE -> L10n.setup_account_type_google(ctx)
        NEXTCLOUD -> L10n.setup_account_type_nextcloud(ctx)
    }

    internal companion object {
        // The kinds the core offers, in its order. It leaves out a browser sign-in this build
        // carries no registration for. Takes the list rather than asking the core, so the JVM
        // suite, which never loads the cdylib, can exercise it.
        internal fun of(kinds: List<ManualSetupKind>): List<AccountKind> = kinds.map { kind ->
            when (kind) {
                ManualSetupKind.PASSWORD -> PASSWORD
                ManualSetupKind.JMAP -> JMAP
                ManualSetupKind.MICROSOFT -> MICROSOFT
                ManualSetupKind.GOOGLE -> GOOGLE
                ManualSetupKind.NEXTCLOUD -> NEXTCLOUD
                ManualSetupKind.DAV -> DAV
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
