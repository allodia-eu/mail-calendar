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
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import uniffi.mailcal_bindings.VideoconferencingPage

// Settings → Videoconferencing's one field: the person's Jitsi server (docs/conferencing.md,
// rule 13). The field sends what it holds each time it is applied or left, and the core decides
// whether that changes anything; the core's page says what is stored and what it last refused.

/** The field's test tag: its label is also the heading of the card it sits in. */
internal const val JITSI_SERVER_FIELD = "jitsi_server"

/**
 * [page] is read again on every signal that moves it, and never replaces what is being typed: the
 * field keeps its own text, starting from the stored server. [onSetJitsiServer] dispatches the text
 * and answers the page as the core left it, which has applied the text by then.
 */
@Composable
internal fun JitsiServerRow(
    page: VideoconferencingPage,
    onSetJitsiServer: (String) -> VideoconferencingPage,
) {
    val ctx = LocalContext.current
    var text by remember { mutableStateOf(page.jitsiServer.orEmpty()) }
    var focused by remember { mutableStateOf(false) }
    // The page a send answered, until the signal it raised brings the next one.
    var answered by remember(page) { mutableStateOf<VideoconferencingPage?>(null) }
    val current = answered ?: page
    // Said while the field still holds exactly the text the core refused, so typing takes it away.
    val refused = current.jitsiRefused != null && current.jitsiRefused == text
    fun send() {
        val after = onSetJitsiServer(text)
        answered = after
        // What was kept reads in its stored form (trimmed, no trailing `/`).
        if (after.jitsiRefused != text) text = after.jitsiServer.orEmpty()
    }
    Column {
        OutlinedTextField(
            value = text,
            onValueChange = { text = it },
            modifier = Modifier
                .fillMaxWidth()
                .testTag(JITSI_SERVER_FIELD)
                .onFocusChanged { state ->
                    if (focused && !state.isFocused) send()
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
            keyboardActions = KeyboardActions(onDone = { send() }),
        )
    }
}
