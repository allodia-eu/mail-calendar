package eu.allodia.mailcal

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType

// Settings → Videoconferencing's one field: the person's Jitsi server (docs/conferencing.md,
// rule 13). Empty until they fill it, and checked by the core before it is stored, so an address it
// refuses is said here and nothing changes.

@Composable
internal fun JitsiServerRow(jitsiServer: () -> String?, onSetJitsiServer: (String) -> Boolean) {
    val ctx = LocalContext.current
    var text by remember { mutableStateOf(jitsiServer().orEmpty()) }
    var refused by remember { mutableStateOf(false) }
    var focused by remember { mutableStateOf(false) }
    // The stored form (its trailing `/` dropped) is what the field shows once it is kept.
    fun store() {
        if (onSetJitsiServer(text)) {
            text = jitsiServer().orEmpty()
            refused = false
        } else {
            refused = true
        }
    }
    Column {
        OutlinedTextField(
            value = text,
            onValueChange = {
                text = it
                refused = false
            },
            modifier = Modifier
                .fillMaxWidth()
                .onFocusChanged { state ->
                    if (focused && !state.isFocused) store()
                    focused = state.isFocused
                },
            singleLine = true,
            label = { Text(L10n.settings_jitsi_server(ctx)) },
            placeholder = { Text("https://meet.example.org") },
            isError = refused,
            supportingText = if (refused) {
                { Text(L10n.settings_jitsi_server_invalid(ctx)) }
            } else {
                null
            },
            keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Uri, imeAction = ImeAction.Done),
            keyboardActions = KeyboardActions(onDone = { store() }),
        )
    }
}
