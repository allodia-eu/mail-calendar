package eu.allodia.mailcal

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.delay

// What the window shows while the core opens the mailbox at launch (docs/boot-sequence.md): a
// blank page, then, once the open has outlasted `afterMillis` (the core's
// `launchStatusAfterMs()`), a progress indicator and what it is waiting for. A normal open ends
// inside the threshold, so most launches never see it.
@Composable
internal fun LaunchStatus(afterMillis: Long) {
    var showsStatus by remember { mutableStateOf(false) }
    LaunchedEffect(Unit) {
        delay(afterMillis)
        showsStatus = true
    }
    if (showsStatus) {
        ConnectionStatus(L10n.status_opening_mailbox(LocalContext.current), isError = false)
    } else {
        Box(modifier = Modifier.fillMaxSize())
    }
}

@Composable
internal fun ConnectionStatus(message: String, isError: Boolean) {
    Box(modifier = Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
        Column(horizontalAlignment = Alignment.CenterHorizontally) {
            if (!isError) {
                CircularProgressIndicator()
                Spacer(modifier = Modifier.height(16.dp))
            }
            Text(
                text = message,
                style = MaterialTheme.typography.bodyLarge,
                color = if (isError) {
                    MaterialTheme.colorScheme.error
                } else {
                    MaterialTheme.colorScheme.onSurface
                },
            )
        }
    }
}
