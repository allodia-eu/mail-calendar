// What is drawn for a calendar or address book an account's sign-in does not reach
// (docs/accounts.md rule 18): the question a switch to the provider asks before the browser opens,
// and the form that gives them a sign-in of their own, as setup's step and as the account page's
// dialog. What each decides is UnreachedRules'; these draw it.
package eu.allodia.mailcal

import android.content.Context
import androidx.activity.compose.BackHandler
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp

internal fun signInUnreachedTitle(ctx: Context, set: UnreachedSet): String = when (set) {
    UnreachedSet.BOTH -> L10n.signin_unreached_title_both(ctx)
    UnreachedSet.CALENDAR -> L10n.signin_unreached_title_calendar(ctx)
    UnreachedSet.CONTACTS -> L10n.signin_unreached_title_contacts(ctx)
}

internal fun separateServerTitle(ctx: Context, set: UnreachedSet): String = when (set) {
    UnreachedSet.BOTH -> L10n.separate_server_title_both(ctx)
    UnreachedSet.CALENDAR -> L10n.separate_server_title_calendar(ctx)
    UnreachedSet.CONTACTS -> L10n.separate_server_title_contacts(ctx)
}

/**
 * Asked before the browser opens: keep the uses on the password (the default), stop using them,
 * which deletes the device's copy and so reads as destructive, or cancel the sign-in. The labels
 * are long, so the actions stack.
 */
@Composable
internal fun SignInUnreachedDialog(set: UnreachedSet, onChoice: (UnreachedChoice) -> Unit) {
    val ctx = LocalContext.current
    AlertDialog(
        onDismissRequest = { onChoice(UnreachedChoice.CANCEL) },
        title = { Text(signInUnreachedTitle(ctx, set)) },
        text = { Text(L10n.signin_unreached_body(ctx)) },
        confirmButton = {
            Column(horizontalAlignment = Alignment.End) {
                TextButton(
                    onClick = { onChoice(UnreachedChoice.KEEP) },
                    modifier = Modifier.testTag("signin-unreached-keep"),
                ) { Text(L10n.signin_unreached_keep(ctx)) }
                TextButton(
                    onClick = { onChoice(UnreachedChoice.STOP) },
                    colors = ButtonDefaults.textButtonColors(contentColor = MaterialTheme.colorScheme.error),
                    modifier = Modifier.testTag("signin-unreached-stop"),
                ) { Text(L10n.signin_unreached_stop(ctx)) }
                TextButton(
                    onClick = { onChoice(UnreachedChoice.CANCEL) },
                    modifier = Modifier.testTag("signin-unreached-cancel"),
                ) { Text(L10n.action_cancel(ctx)) }
            }
        },
    )
}

/** What the form holds while it is on screen: the login, prefilled, and the password. */
private class SeparateFields(username: String) {
    var username by mutableStateOf(username)
    var password by mutableStateOf("")
    val ready: Boolean get() = username.isNotBlank() && password.isNotEmpty()
}

@Composable
private fun SeparateServerBody(form: SeparateServerForm, fields: SeparateFields, error: String?) {
    val ctx = LocalContext.current
    Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
        Text(L10n.separate_server_body(ctx, form.host), style = MaterialTheme.typography.bodyMedium)
        SetupField(
            fields.username,
            { fields.username = it },
            L10n.setup_field_username(ctx),
            keyboardType = KeyboardType.Email,
        )
        PasswordField(fields.password, { fields.password = it }, L10n.setup_field_password(ctx))
        error?.let {
            Text(
                it,
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.error,
                modifier = Modifier.testTag("separate-server-error"),
            )
        }
    }
}

@Composable
private fun ConnectLabel(busy: Boolean) {
    val ctx = LocalContext.current
    Row(verticalAlignment = Alignment.CenterVertically) {
        if (busy) {
            CircularProgressIndicator(modifier = Modifier.size(18.dp))
            Spacer(modifier = Modifier.width(8.dp))
        }
        Text(L10n.action_connect(ctx))
    }
}

/**
 * Setup's step, after the mailbox connected and before its link step. Skip, and back, switch the
 * uses off, so the account does not open on a calendar that cannot connect.
 */
@Composable
internal fun SeparateServerStepScreen(
    form: SeparateServerForm,
    busy: Boolean,
    error: String?,
    onConnect: (username: String, password: String) -> Unit,
    onSkip: () -> Unit,
) {
    val ctx = LocalContext.current
    val fields = remember(form) { SeparateFields(form.username) }
    BackHandler(enabled = !busy) { onSkip() }
    StepColumn {
        Text(separateServerTitle(ctx, form.set), style = MaterialTheme.typography.headlineSmall)
        SeparateServerBody(form, fields, error)
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp), modifier = Modifier.fillMaxWidth()) {
            OutlinedButton(
                onClick = onSkip,
                enabled = !busy,
                modifier = Modifier.weight(1f).testTag("separate-server-skip"),
            ) { Text(L10n.separate_server_skip(ctx)) }
            Button(
                onClick = { onConnect(fields.username.trim(), fields.password) },
                enabled = !busy && fields.ready,
                modifier = Modifier.weight(1f).testTag("separate-server-connect"),
            ) { ConnectLabel(busy) }
        }
    }
}

/** The account page's dialog for the same form, with Cancel in place of Skip. */
@Composable
internal fun SeparateServerDialog(
    form: SeparateServerForm,
    busy: Boolean,
    error: String?,
    onConnect: (username: String, password: String) -> Unit,
    onDismiss: () -> Unit,
) {
    val ctx = LocalContext.current
    val fields = remember(form) { SeparateFields(form.username) }
    AlertDialog(
        onDismissRequest = { if (!busy) onDismiss() },
        title = { Text(separateServerTitle(ctx, form.set)) },
        text = { SeparateServerBody(form, fields, error) },
        confirmButton = {
            TextButton(
                onClick = { onConnect(fields.username.trim(), fields.password) },
                enabled = !busy && fields.ready,
                modifier = Modifier.testTag("separate-server-connect"),
            ) { ConnectLabel(busy) }
        },
        dismissButton = {
            TextButton(onClick = onDismiss, enabled = !busy, modifier = Modifier.testTag("separate-server-cancel")) {
                Text(L10n.action_cancel(ctx))
            }
        },
    )
}
