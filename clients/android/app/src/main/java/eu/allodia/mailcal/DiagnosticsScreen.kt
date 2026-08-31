// The Diagnostics screen (Settings → Diagnostics): the in-app surface over the rotating file log
// (docs/logging.md). It exists because on a release build the log was unreachable
// without a cable and four adb steps, which defeats the point of an attachable log. From here a
// user can, with no cable: see the log's size/rotation state, read the current app.log in place,
// share it through the system share sheet (the "attach to a support request" flow the logging
// contract is written for), copy its absolute path, and opt into DEBUG detail for a support
// session. The log itself is privacy-safe by construction (counts, ids, durations, events:
// never mail/event content, addresses, or credentials), which is what makes sharing it safe;
// the share flow still surfaces that one-line reminder in a confirm step BEFORE the file leaves
// the device. The debug choice is persisted (DiagnosticsPrefs) and re-applied to every core at
// boot, including the background worker's own.
package eu.allodia.mailcal

import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import android.text.format.Formatter
import android.widget.Toast
import androidx.activity.compose.BackHandler
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.systemBarsPadding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.unit.dp
import androidx.core.content.FileProvider
import java.io.File
import kotlinx.coroutines.launch
import uniffi.mailcal_bindings.LogLevel

@Composable
internal fun DiagnosticsScreen(
    // Applies a new ceiling to the RUNNING core (MailcalApp.setLogLevel); persistence is this
    // screen's job (DiagnosticsPrefs), so the two can never disagree about who owns what.
    onSetLogLevel: (LogLevel) -> Unit,
    onBack: () -> Unit,
) {
    // The viewer swaps in over the settings list (like Settings swaps in over the mailbox);
    // its Back returns here, not to Settings.
    var viewingLog by remember { mutableStateOf(false) }
    // System back mirrors Done, one level at a time: the viewer returns to this screen, this screen
    // to Settings. Without it, back left the app outright from two screens deep in Settings.
    BackHandler {
        if (viewingLog) viewingLog = false else onBack()
    }
    if (viewingLog) {
        LogViewer(onBack = { viewingLog = false })
    } else {
        DiagnosticsSettings(
            onSetLogLevel = onSetLogLevel,
            onViewLog = { viewingLog = true },
            onBack = onBack,
        )
    }
}

@Composable
private fun DiagnosticsSettings(
    onSetLogLevel: (LogLevel) -> Unit,
    onViewLog: () -> Unit,
    onBack: () -> Unit,
) {
    val ctx = LocalContext.current
    var confirmingShare by remember { mutableStateOf(false) }
    // One snapshot per entry to the screen: the log grows while it's open, but a live ticker
    // would be noise for a number whose point is the order of magnitude.
    val snapshot = remember { FileLog.snapshot() }

    // Outside the Scaffold, so the system bars are this screen's own problem (see WelcomeScreen).
    Column(modifier = Modifier.fillMaxSize().systemBarsPadding().padding(16.dp)) {
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Text(L10n.settings_category_diagnostics(ctx), style = MaterialTheme.typography.titleLarge)
            TextButton(onClick = onBack) { Text(L10n.action_done(ctx)) }
        }
        Column(modifier = Modifier.fillMaxWidth().verticalScroll(rememberScrollState())) {
            SettingsGroupCard(L10n.diagnostics_log_heading(ctx), L10n.diagnostics_log_description(ctx)) {
                StatusRow(
                    L10n.diagnostics_log_size_label(ctx),
                    Formatter.formatShortFileSize(ctx, snapshot?.totalBytes ?: 0L),
                )
                StatusRow(
                    L10n.diagnostics_log_backups_label(ctx),
                    (snapshot?.backupCount ?: 0).toString(),
                )
                Text(
                    L10n.diagnostics_log_cap_note(ctx),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.padding(top = 4.dp, bottom = 4.dp),
                )
                TextButton(onClick = onViewLog) { Text(L10n.diagnostics_view_log(ctx)) }
                // Sharing goes through the privacy-note confirm below, never straight to the sheet.
                TextButton(onClick = { confirmingShare = true }) { Text(L10n.diagnostics_share_log(ctx)) }
                TextButton(onClick = { copyLogPath(ctx) }) { Text(L10n.diagnostics_copy_path(ctx)) }
            }
            Spacer(modifier = Modifier.height(8.dp))
            SettingsGroupCard(
                L10n.diagnostics_debug_heading(ctx),
                L10n.diagnostics_debug_description(ctx),
            ) {
                DebugToggle(onSetLogLevel)
            }
            Spacer(modifier = Modifier.height(24.dp))
        }
    }

    // The privacy note rides the confirm step so the user reads what the file contains BEFORE it
    // leaves the device, the share sheet only opens from the confirm action.
    if (confirmingShare) {
        AlertDialog(
            onDismissRequest = { confirmingShare = false },
            title = { Text(L10n.diagnostics_share_confirm_title(ctx)) },
            text = { Text(L10n.diagnostics_share_privacy_note(ctx)) },
            confirmButton = {
                TextButton(
                    onClick = {
                        confirmingShare = false
                        shareLog(ctx)
                    },
                ) {
                    Text(L10n.diagnostics_share_log(ctx))
                }
            },
            dismissButton = {
                TextButton(onClick = { confirmingShare = false }) { Text(L10n.action_cancel(ctx)) }
            },
        )
    }
}

// The read-only viewer over the CURRENT app.log. Newest entries are last (the file's own append
// order), and the list opens scrolled to the end, the most recent lines are what a support
// session is about. Scrolling away reveals a jump-to-end affordance. The read is synchronous:
// rotation caps the file at ~1 MB, so it is a few milliseconds, and keeping it in `remember`
// keeps the screen trivially testable.
@Composable
private fun LogViewer(onBack: () -> Unit) {
    val ctx = LocalContext.current
    val lines = remember {
        FileLog.readCurrent().orEmpty().lines().dropLastWhile { it.isEmpty() }
    }
    val listState = rememberLazyListState(
        initialFirstVisibleItemIndex = (lines.size - 1).coerceAtLeast(0),
    )
    val scope = rememberCoroutineScope()

    // Outside the Scaffold, so the system bars are this screen's own problem (see WelcomeScreen).
    Column(modifier = Modifier.fillMaxSize().systemBarsPadding().padding(16.dp)) {
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Text(L10n.diagnostics_view_log(ctx), style = MaterialTheme.typography.titleLarge)
            TextButton(onClick = onBack) { Text(L10n.action_done(ctx)) }
        }
        if (lines.isEmpty()) {
            Box(modifier = Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
                Text(
                    L10n.diagnostics_log_empty(ctx),
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        } else {
            Box(modifier = Modifier.weight(1f).fillMaxWidth()) {
                LazyColumn(state = listState, modifier = Modifier.fillMaxSize()) {
                    items(lines.size) { i ->
                        Text(
                            lines[i],
                            fontFamily = FontFamily.Monospace,
                            style = MaterialTheme.typography.bodySmall,
                        )
                    }
                }
                // Shown only while scrolled away from the end (at the end there is nothing left
                // to scroll forward to, so the affordance disappears rather than lying).
                if (listState.canScrollForward) {
                    Button(
                        onClick = { scope.launch { listState.scrollToItem(lines.size - 1) } },
                        modifier = Modifier.align(Alignment.BottomEnd).padding(8.dp),
                    ) {
                        Text(L10n.diagnostics_jump_to_end(ctx))
                    }
                }
            }
        }
    }
}

// The "include more detail" toggle: ON raises the live core to DEBUG, OFF returns it to the INFO
// default, applied immediately (setLogLevel) and persisted for every core built at the next
// boot, including the background worker's headless one (DiagnosticsPrefs.bootLogLevel).
@Composable
private fun DebugToggle(onSetLogLevel: (LogLevel) -> Unit) {
    val ctx = LocalContext.current
    var enabled by remember { mutableStateOf(DiagnosticsPrefs.debugEnabled(ctx)) }
    Row(
        modifier = Modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.End,
    ) {
        Switch(
            checked = enabled,
            onCheckedChange = {
                enabled = it
                DiagnosticsPrefs.setDebugEnabled(ctx, it)
                onSetLogLevel(logLevelForDebug(it))
            },
        )
    }
}

// A label/value status row (log size, backup count).
@Composable
private fun StatusRow(label: String, value: String) {
    Row(
        modifier = Modifier.fillMaxWidth().padding(vertical = 2.dp),
        horizontalArrangement = Arrangement.SpaceBetween,
    ) {
        Text(label, style = MaterialTheme.typography.bodyMedium)
        Text(
            value,
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
    }
}

// The raw ACTION_SEND the share sheet is opened over, factored out so a test can pin the
// action/type/stream/read-grant without launching a chooser. The FileProvider authority plus the
// `files-path logs/` root in res/xml/file_paths.xml are what let the share target read the
// app-private file; getUriForFile throws for a file outside a declared root, so the test also
// covers that configuration.
internal fun buildLogShareIntent(ctx: Context, logFile: File): Intent {
    val uri = FileProvider.getUriForFile(ctx, "${ctx.packageName}.fileprovider", logFile)
    return Intent(Intent.ACTION_SEND).apply {
        type = "text/plain"
        putExtra(Intent.EXTRA_STREAM, uri)
        addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
    }
}

// Fires the system share sheet over the current log file. Only ever reached from the confirm
// step above, the privacy note has been shown by the time this runs.
private fun shareLog(ctx: Context) {
    val path = FileLog.snapshot()?.path ?: return
    try {
        val send = buildLogShareIntent(ctx, File(path))
        ctx.startActivity(Intent.createChooser(send, L10n.diagnostics_share_log(ctx)))
    } catch (_: Exception) {
        // A vanished file or a misconfigured provider root: say so rather than crash or stay mute.
        Toast.makeText(ctx, L10n.error_unknown(ctx), Toast.LENGTH_SHORT).show()
    }
}

// Copies the log's absolute path for power users (`adb pull`, a bug report). The toast is the
// contract's transient feedback; Android 13+ also shows its own clipboard confirmation, and the
// duplication is harmless.
private fun copyLogPath(ctx: Context) {
    val path = FileLog.snapshot()?.path ?: return
    val clipboard = ctx.getSystemService(Context.CLIPBOARD_SERVICE) as? ClipboardManager ?: return
    clipboard.setPrimaryClip(ClipData.newPlainText("log path", path))
    Toast.makeText(ctx, L10n.diagnostics_path_copied(ctx), Toast.LENGTH_SHORT).show()
}
