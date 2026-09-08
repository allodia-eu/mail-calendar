// The "your name" step: the one question asked once an account connects (docs/sending.md).
//
// After the add rather than before it. The screen that adds an account is the address field and
// nothing else (docs/onboarding.md), and only a connected account can be asked what its provider
// already calls this person, which on a Microsoft or Google account turns the step into a
// confirmation.
//
// The state is a plain class rather than a knot of `remember`s so the JVM suite can drive it
// without composing a screen (AGENTS.md).
package eu.allodia.mailcal

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp

/**
 * What the step is showing for one account: the text being edited, and whether the suggestion is
 * still being fetched.
 *
 * [account] is fixed for the life of the step. Asking "your name" without knowing whose would
 * write the answer onto whichever account happened to be first, so a route that cannot name the
 * account it added opens no step at all.
 */
internal class SenderNameStepState(val account: String) {
    /** The name being edited. Empty is a real answer: the account sends as a bare address. */
    var name by mutableStateOf("")

    /**
     * True until the suggestion has arrived. The lookup is a provider round trip, so the field
     * stays disabled rather than accepting typing that is about to be replaced.
     */
    var loading by mutableStateOf(true)
        private set

    /**
     * Adopts the provider's suggestion, unless the user has already typed something.
     *
     * The guard is what makes a slow provider harmless: the answer can arrive after somebody has
     * started typing, and overwriting them then is worse than never having asked.
     */
    fun suggested(value: String) {
        if (name.isEmpty()) {
            name = value
        }
        loading = false
    }
}

/**
 * The step itself: a dialog over the running app, raised once an account connects.
 *
 * Not cancellable by tapping outside. Both ways out are buttons, because "skip" is a real answer
 * the app has to be correct under, and a stray tap is not that answer.
 */
@androidx.compose.runtime.Composable
internal fun SenderNameStepDialog(
    account: String?,
    suggest: suspend (String) -> String,
    onSave: (account: String, name: String) -> Unit,
    onDismiss: () -> Unit,
) {
    if (account == null) return
    val ctx = androidx.compose.ui.platform.LocalContext.current
    val state = androidx.compose.runtime.remember(account) { SenderNameStepState(account) }
    androidx.compose.runtime.LaunchedEffect(account) { state.suggested(suggest(account)) }

    androidx.compose.material3.AlertDialog(
        onDismissRequest = {},
        properties =
            androidx.compose.ui.window.DialogProperties(
                dismissOnBackPress = false,
                dismissOnClickOutside = false,
            ),
        title = { androidx.compose.material3.Text(L10n.setup_sender_name_title(ctx)) },
        text = {
            Column {
                androidx.compose.material3.Text(L10n.setup_sender_name_description(ctx))
                Spacer(modifier = Modifier.height(12.dp))
                androidx.compose.material3.OutlinedTextField(
                    value = state.name,
                    onValueChange = { state.name = it },
                    singleLine = true,
                    enabled = !state.loading,
                    label = { androidx.compose.material3.Text(L10n.setup_sender_name_field(ctx)) },
                    modifier = Modifier.fillMaxWidth(),
                )
            }
        },
        confirmButton = {
            androidx.compose.material3.TextButton(
                onClick = { onSave(account, state.name) },
                enabled = !state.loading,
            ) {
                androidx.compose.material3.Text(L10n.setup_sender_name_continue(ctx))
            }
        },
        dismissButton = {
            androidx.compose.material3.TextButton(onClick = onDismiss) {
                androidx.compose.material3.Text(L10n.setup_sender_name_skip(ctx))
            }
        },
    )
}
