// What is drawn for a calendar or address book an account's sign-in does not reach
// (docs/accounts.md rule 18): the question a switch to the provider asks before the browser opens,
// and the form that gives them a sign-in of their own, as setup's step and as the account page's
// dialog, with the sign-in at the server's own provider when it offers one (docs/mail-oauth.md
// rule 13). What each decides is UnreachedRules' and SeparateSignIn's; these draw it.
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
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
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

/**
 * The sign-in at the server's own provider, above the fields once the core has said it is offered,
 * with its note beneath. The fields stay below it, so one being typed in keeps its focus.
 */
@Composable
private fun SeparateSignInOffer(controls: SeparateControls, onSignIn: () -> Unit) {
    if (!controls.offersSignIn) return
    val ctx = LocalContext.current
    Button(
        onClick = onSignIn,
        enabled = controls.editable,
        modifier = Modifier.fillMaxWidth().testTag("separate-server-sign-in"),
    ) {
        if (controls.signingIn) {
            CircularProgressIndicator(modifier = Modifier.size(18.dp))
        } else {
            Text(L10n.setup_imap_signin_button(ctx))
        }
    }
    Text(
        L10n.setup_imap_signin_note(ctx),
        style = MaterialTheme.typography.bodySmall,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
    )
}

@Composable
private fun SeparateServerBody(
    form: SeparateServerForm,
    fields: SeparateFields,
    controls: SeparateControls,
    error: String?,
    onSignIn: () -> Unit,
) {
    val ctx = LocalContext.current
    Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
        Text(L10n.separate_server_body(ctx, form.host), style = MaterialTheme.typography.bodyMedium)
        SeparateSignInOffer(controls, onSignIn)
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

/**
 * Asks once, when the form is drawn, whether its server offers the sign-in, and retires a sign-in
 * still out for it when the form leaves the screen.
 */
@Composable
private fun SeparateFormEffects(form: SeparateServerForm, onShown: () -> Unit, onLeave: () -> Unit) {
    LaunchedEffect(form) { onShown() }
    DisposableEffect(form) { onDispose(onLeave) }
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
    controls: SeparateControls,
    error: String?,
    onConnect: (username: String, password: String) -> Unit,
    onSkip: () -> Unit,
    onSignIn: () -> Unit = {},
    onShown: () -> Unit = {},
    onLeave: () -> Unit = {},
) {
    val ctx = LocalContext.current
    val fields = remember(form) { SeparateFields(form.username) }
    SeparateFormEffects(form, onShown, onLeave)
    BackHandler(enabled = controls.leavable) { onSkip() }
    StepColumn {
        Text(separateServerTitle(ctx, form.set), style = MaterialTheme.typography.headlineSmall)
        SeparateServerBody(form, fields, controls, error, onSignIn)
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp), modifier = Modifier.fillMaxWidth()) {
            OutlinedButton(
                onClick = onSkip,
                enabled = controls.leavable,
                modifier = Modifier.weight(1f).testTag("separate-server-skip"),
            ) { Text(L10n.separate_server_skip(ctx)) }
            Button(
                onClick = { onConnect(fields.username.trim(), fields.password) },
                enabled = controls.editable && fields.ready,
                modifier = Modifier.weight(1f).testTag("separate-server-connect"),
            ) { ConnectLabel(controls.connecting) }
        }
    }
}

/** The account page's dialog for the same form, with Cancel in place of Skip. */
@Composable
internal fun SeparateServerDialog(
    form: SeparateServerForm,
    controls: SeparateControls,
    error: String?,
    onConnect: (username: String, password: String) -> Unit,
    onDismiss: () -> Unit,
    onSignIn: () -> Unit = {},
    onShown: () -> Unit = {},
    onLeave: () -> Unit = {},
) {
    val ctx = LocalContext.current
    val fields = remember(form) { SeparateFields(form.username) }
    SeparateFormEffects(form, onShown, onLeave)
    AlertDialog(
        onDismissRequest = { if (controls.leavable) onDismiss() },
        title = { Text(separateServerTitle(ctx, form.set)) },
        text = { SeparateServerBody(form, fields, controls, error, onSignIn) },
        confirmButton = {
            TextButton(
                onClick = { onConnect(fields.username.trim(), fields.password) },
                enabled = controls.editable && fields.ready,
                modifier = Modifier.testTag("separate-server-connect"),
            ) { ConnectLabel(controls.connecting) }
        },
        dismissButton = {
            TextButton(
                onClick = onDismiss,
                enabled = controls.leavable,
                modifier = Modifier.testTag("separate-server-cancel"),
            ) { Text(L10n.action_cancel(ctx)) }
        },
    )
}
