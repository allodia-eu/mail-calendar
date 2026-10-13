// The Outbox branch of the mailbox tab: the queued list (docs/sending.md). The composer a withdrawn
// message comes back into is MainActivityComposer.kt's: Edit is pressed on the list, and what
// answers it is a composer the list no longer holds anything for.
package eu.allodia.mailcal

import androidx.compose.material3.DrawerState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.rememberCoroutineScope
import kotlinx.coroutines.launch
import uniffi.mailcal_bindings.Intent
import uniffi.mailcal_bindings.MailcalApp

/** Every account's unsent messages, and what each one offers. */
@Composable
internal fun MainActivity.OutboxPane(instance: MailcalApp, drawerState: DrawerState) {
    val scope = rememberCoroutineScope()
    OutboxScreen(
        queued = outbox,
        accounts = accounts,
        onOpenDrawer = { scope.launch { drawerState.open() } },
        onAct = { account, op, action ->
            // The composition is not known until the core answers, so the directory is named
            // for this request alone.
            val staging = composerStagingDirectory(newComposition()).absolutePath
            instance.dispatch(Intent.Outbox(outboxIntent(account, op, action, staging)))
        },
    )
}
