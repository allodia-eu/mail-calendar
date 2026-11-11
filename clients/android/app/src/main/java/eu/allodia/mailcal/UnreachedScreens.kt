// What is drawn for a calendar or address book an account's sign-in does not reach
// (docs/accounts.md rule 18): the question a switch to the provider asks before the browser opens,
// and the form that gives them a sign-in of their own, as setup's step and as the account page's
// dialog, with the sign-in in the browser when the server offers one (docs/mail-oauth.md rule 13).
// What each decides is the core's, read by UnreachedRules and SeparateSignIn; these draw it.
package eu.allodia.mailcal

import android.content.Context
import androidx.activity.compose.BackHandler
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import uniffi.mailcal_bindings.BrowserSignIn

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

/** The line under the sign-in: whose it is, or, while the browser holds it, that it waits there as [username]. */
internal fun separateSignInNote(ctx: Context, controls: SeparateControls, username: String): String = when {
    controls.nextcloud && controls.waiting != null -> L10n.nextcloud_sign_in_waiting(ctx)
    controls.nextcloud -> L10n.nextcloud_sign_in_note(ctx)
    controls.waiting != null -> L10n.settings_account_signin_waiting(ctx, username)
    else -> L10n.setup_imap_signin_note(ctx)
}

/**
 * The sign-in in the browser, above the fields once the core has said it is offered, with its
 * line beneath and, while the browser holds it, its Cancel. The fields stay below it, so one
 * being typed in keeps its focus.
 */
@Composable
private fun SeparateSignInOffer(
    form: SeparateServerForm,
    controls: SeparateControls,
    onSignIn: () -> Unit,
    onCancelSignIn: (BrowserSignIn) -> Unit,
) {
    if (!controls.offersSignIn) return
    val ctx = LocalContext.current
    ControlButton(
        label = if (controls.nextcloud) L10n.nextcloud_sign_in_button(ctx) else L10n.setup_imap_signin_button(ctx),
        control = controls.signIn,
        tag = "separate-server-sign-in",
        modifier = Modifier.fillMaxWidth(),
        onClick = onSignIn,
    )
    Text(
        separateSignInNote(ctx, controls, form.section.username),
        style = MaterialTheme.typography.bodySmall,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
    )
    controls.waiting?.let { browser ->
        ControlTextButton(L10n.action_cancel(ctx), browser.cancel, "separate-server-sign-in-cancel") {
            onCancelSignIn(browser)
        }
    }
}

@Composable
private fun SeparateServerBody(
    form: SeparateServerForm,
    fields: SeparateFields,
    controls: SeparateControls,
    error: String?,
    onSignIn: () -> Unit,
    onCancelSignIn: (BrowserSignIn) -> Unit,
) {
    val ctx = LocalContext.current
    Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
        Text(L10n.separate_server_body(ctx, form.section.host), style = MaterialTheme.typography.bodyMedium)
        SeparateSignInOffer(form, controls, onSignIn, onCancelSignIn)
        HeldFields(controls.connect) { open ->
            SetupField(
                fields.username,
                { fields.username = it },
                L10n.setup_field_username(ctx),
                keyboardType = KeyboardType.Email,
                enabled = open,
            )
            PasswordField(fields.password, { fields.password = it }, L10n.setup_field_password(ctx), enabled = open)
        }
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
 * Cancels a sign-in still out for the form when it leaves the screen, read off the section last
 * drawn: the form may already be gone from its holder by then.
 */
@Composable
private fun SeparateFormEffects(form: SeparateServerForm, onCancelSignIn: (BrowserSignIn) -> Unit) {
    val drawn by rememberUpdatedState(form)
    val cancel by rememberUpdatedState(onCancelSignIn)
    DisposableEffect(form.accountId) { onDispose { drawn.section.browser?.let(cancel) } }
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
    onCancelSignIn: (BrowserSignIn) -> Unit = {},
) {
    val ctx = LocalContext.current
    // Kept across the core's redraws of the form, which change its section.
    val fields = remember(form.accountId) { SeparateFields(form.section.username) }
    SeparateFormEffects(form, onCancelSignIn)
    BackHandler(enabled = controls.leavable) { onSkip() }
    StepColumn {
        Text(separateServerTitle(ctx, form.set), style = MaterialTheme.typography.headlineSmall)
        SeparateServerBody(form, fields, controls, error, onSignIn, onCancelSignIn)
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp), modifier = Modifier.fillMaxWidth()) {
            ControlTextButton(
                L10n.separate_server_skip(ctx),
                controls.skip,
                "separate-server-skip",
                modifier = Modifier.weight(1f),
                onClick = onSkip,
            )
            ControlButton(
                L10n.action_connect(ctx),
                controls.connect,
                "separate-server-connect",
                modifier = Modifier.weight(1f),
                ready = fields.ready,
            ) { onConnect(fields.username.trim(), fields.password) }
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
    onCancelSignIn: (BrowserSignIn) -> Unit = {},
) {
    val ctx = LocalContext.current
    val fields = remember(form.accountId) { SeparateFields(form.section.username) }
    SeparateFormEffects(form, onCancelSignIn)
    AlertDialog(
        onDismissRequest = { if (controls.leavable) onDismiss() },
        title = { Text(separateServerTitle(ctx, form.set)) },
        text = { SeparateServerBody(form, fields, controls, error, onSignIn, onCancelSignIn) },
        confirmButton = {
            ControlTextButton(
                L10n.action_connect(ctx),
                controls.connect.copy(enabled = controls.connect.enabled && fields.ready),
                "separate-server-connect",
            ) { onConnect(fields.username.trim(), fields.password) }
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
