// An account's page, its sign-in details: the servers and password of an account that signs in
// with a password to servers of its own (docs/accounts.md rule 14), and the pasted password or API
// token of a JMAP account whose server stopped accepting it.
//
// Saving tries them before keeping them (update_account_endpoints, replace_account_secret), so a
// typo never costs a working account, and an expired password is replaced here.
package eu.allodia.mailcal

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import uniffi.mailcal_bindings.AccountEndpoints
import uniffi.mailcal_bindings.AccountEntry
import uniffi.mailcal_bindings.ConnectionSecurity
import uniffi.mailcal_bindings.MailServerKind

/** One server's host as typed, and its port and security. */
private data class EditedServer(val host: String, val field: ManualServerField)

private fun editedServer(
    address: String?,
    security: ConnectionSecurity,
    standard: ((ConnectionSecurity) -> Int)?,
): EditedServer? = address?.let {
    EditedServer(ManualServerField.splitHost(it).first, ManualServerField(standard).adoptDetected(it, security))
}

/**
 * The servers the account has, its login and a new password. What is typed is remembered against
 * the account and what it was drawn from, so a redraw the core asks for leaves it alone and a
 * saved change starts the form again from what was saved.
 */
@Composable
internal fun ServersGroup(
    entry: AccountEntry,
    endpoints: AccountEndpoints,
    expired: Boolean,
    ui: AccountsSettingsUi,
) {
    val ctx = LocalContext.current
    val imapPort = ui.standardPort?.let { lookup -> { s: ConnectionSecurity -> lookup(MailServerKind.IMAP, s) } }
    val smtpPort = ui.standardPort?.let { lookup -> { s: ConnectionSecurity -> lookup(MailServerKind.SMTP, s) } }
    var imap by remember(entry.id, endpoints, ui.state.serversSaved) {
        mutableStateOf(editedServer(endpoints.imapHost, endpoints.imapSecurity, imapPort))
    }
    var smtp by remember(entry.id, endpoints, ui.state.serversSaved) {
        mutableStateOf(editedServer(endpoints.smtpHost, endpoints.smtpSecurity, smtpPort))
    }
    var caldav by remember(entry.id, endpoints, ui.state.serversSaved) { mutableStateOf(endpoints.caldavUrl) }
    var carddav by remember(entry.id, endpoints, ui.state.serversSaved) { mutableStateOf(endpoints.carddavUrl) }
    var login by remember(entry.id, endpoints, ui.state.serversSaved) { mutableStateOf(endpoints.username) }
    var password by remember(entry.id, endpoints, ui.state.serversSaved) { mutableStateOf("") }
    val fields = EndpointFields(
        imapHost = imap?.let {
            AccountEndpointEdit.joined(it.host, it.field.port, it.field.security, imapPort)
        },
        imapSecurity = imap?.field?.security,
        smtpHost = smtp?.let {
            AccountEndpointEdit.joined(it.host, it.field.port, it.field.security, smtpPort)
        },
        smtpSecurity = smtp?.field?.security,
        caldavUrl = caldav?.trim(),
        carddavUrl = carddav?.trim(),
        username = login.trim(),
        password = password,
    )
    val saving = ui.state.savingServers == entry.id
    val description = if (expired) {
        L10n.signin_expired_prompt(ctx, accountName(ctx, entry))
    } else {
        L10n.settings_account_servers_description(ctx)
    }
    SettingsGroupCard(L10n.settings_account_servers_heading(ctx), description) {
        Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
            imap?.let { server ->
                ServerRow(
                    host = server.host,
                    onHostChange = { imap = server.copy(host = it) },
                    label = L10n.settings_account_field_imap(ctx),
                    placeholder = null,
                    field = server.field,
                    onFieldChange = { imap = server.copy(field = it) },
                    ctx = ctx,
                )
            }
            smtp?.let { server ->
                ServerRow(
                    host = server.host,
                    onHostChange = { smtp = server.copy(host = it) },
                    label = L10n.setup_field_smtp(ctx),
                    placeholder = null,
                    field = server.field,
                    onFieldChange = { smtp = server.copy(field = it) },
                    ctx = ctx,
                )
            }
            caldav?.let { url ->
                SetupField(url, { caldav = it }, L10n.setup_field_caldav(ctx), keyboardType = KeyboardType.Uri)
            }
            carddav?.let { url ->
                SetupField(url, { carddav = it }, L10n.settings_account_field_carddav(ctx), keyboardType = KeyboardType.Uri)
            }
            SetupField(login, { login = it }, L10n.settings_account_field_login(ctx))
            PasswordField(password, { password = it }, L10n.settings_account_field_new_password(ctx))
            Button(
                onClick = {
                    ui.onSaveServers(entry.id, AccountEndpointEdit.edited(endpoints, fields))
                },
                // Waits while a server the account has is left empty, rather than sending it for
                // the core to refuse.
                enabled = !saving && AccountEndpointEdit.complete(endpoints, fields),
                modifier = Modifier.testTag("account-servers-save"),
            ) {
                if (saving) {
                    CircularProgressIndicator(modifier = Modifier.size(18.dp))
                    Spacer(modifier = Modifier.width(8.dp))
                    androidx.compose.material3.Text(L10n.settings_account_servers_trying(ctx))
                } else {
                    androidx.compose.material3.Text(L10n.action_save(ctx))
                }
            }
        }
    }
}

/**
 * A new password or API token for a JMAP account whose pasted one the server stopped accepting,
 * inside the account's Sign-in section. Offered only while it is refused: a working secret stays
 * out of ordinary settings (docs/settings.md).
 */
@Composable
internal fun SecretFields(entry: AccountEntry, ui: AccountsSettingsUi) {
    val ctx = LocalContext.current
    var secret by remember(entry.id) { mutableStateOf("") }
    val replacing = ui.state.replacingSecret == entry.id
    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
        androidx.compose.material3.Text(
            L10n.signin_expired_prompt(ctx, accountName(ctx, entry)),
            style = androidx.compose.material3.MaterialTheme.typography.bodyMedium,
        )
        PasswordField(secret, { secret = it }, L10n.setup_jmap_secret_placeholder(ctx))
        Button(
            onClick = { ui.onReplaceSecret(entry.id, secret) },
            enabled = !replacing && secret.isNotBlank(),
            modifier = Modifier.testTag("account-secret-save"),
        ) {
            if (replacing) {
                CircularProgressIndicator(modifier = Modifier.size(18.dp))
                Spacer(modifier = Modifier.width(8.dp))
            }
            androidx.compose.material3.Text(L10n.signin_expired_action(ctx))
        }
    }
}
