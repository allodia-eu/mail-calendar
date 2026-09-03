// The in-app account-setup form for the Android client, shown on first run when the
// OS secure store holds no config yet. It collects the mail-server endpoints + credentials,
// hands them to the Rust core (accountConfigToml / jmapAccountConfigToml) which validates them
// and returns the config TOML to store, then starts the connect. Replaces the old plaintext
// account.toml seed file. Split into its own file to keep MainActivity under the 500-line limit.
package eu.allodia.mailcal

import android.content.Context
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.systemBarsPadding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.Checkbox
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.SegmentedButton
import androidx.compose.material3.SegmentedButtonDefaults
import androidx.compose.material3.SingleChoiceSegmentedButtonRow
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalUriHandler
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.unit.dp
import uniffi.mailcal_bindings.AccountSetup
import uniffi.mailcal_bindings.JmapSetup
import uniffi.mailcal_bindings.OAuthRoutes

// The account kinds the setup form can offer (the two OAuth providers, Microsoft and Google, sit
// together at the end); which of them this build shows is [AccountKind.offered].
internal enum class AccountKind {
    PASSWORD,
    JMAP,
    MICROSOFT,
    GOOGLE,
    ;

    internal fun label(ctx: Context): String = when (this) {
        PASSWORD -> L10n.setup_account_type_password(ctx)
        JMAP -> L10n.setup_account_type_jmap(ctx)
        MICROSOFT -> L10n.setup_account_type_microsoft(ctx)
        GOOGLE -> L10n.setup_account_type_google(ctx)
    }

    internal companion object {
        // The kinds a build offering [routes] shows, in picker order. A browser sign-in needs the
        // provider's OAuth client registration, which is injected at build time, so a build given
        // none drops the route rather than showing a button that fails at the provider. The two
        // credential routes are always there.
        //
        // Takes the routes rather than asking the core for them, so this is a pure function the
        // JVM suite can exercise, asking would load the cdylib, which no test here does.
        internal fun offered(routes: OAuthRoutes): List<AccountKind> = entries.filter { kind ->
            when (kind) {
                MICROSOFT -> routes.microsoft
                GOOGLE -> routes.google
                PASSWORD, JMAP -> true
            }
        }
    }
}

// How long typing must pause before we probe a JMAP server for OAuth support. The probe is
// several network round trips, so firing it per keystroke would hammer half-typed domains.
internal const val JMAP_SIGNIN_PROBE_DEBOUNCE_MS = 600L

// The manual setup form (the 3-tab picker). Reached on first run, when adding another account,
// or as the "Set up manually" escape from the email-first detection flow, which prefills the
// email (and, when it found servers the user chose to edit, the host fields + starting tab) and
// shows [note] explaining why detection routed here.
//
// [onConnect] receives the assembled IMAP AccountSetup and [onConnectJmap] the JMAP JmapSetup;
// each validates via the core and starts the connect, returning null on success or an error
// message to display inline (the core rejects empty/invalid fields). For IMAP the four login
// fields are required; SMTP and CalDAV are optional and passed as null when blank. For JMAP the
// email plus the secret are required, one field, since a password and an API token are
// interchangeable to the core; the server is optional (the core derives it from the email
// domain). [onCancel], when non-null, shows a Cancel button, used
// when adding another account (the first account can't be cancelled).
@androidx.compose.runtime.Composable
internal fun AccountSetupScreen(
    externalError: String? = null,
    onCancel: (() -> Unit)? = null,
    signingIn: Boolean = false,
    signingInGoogle: Boolean = false,
    connecting: Boolean = false,
    onSignInMicrosoft: (String?) -> Unit = {},
    onSignInGoogle: (String?) -> Unit = {},
    onConnect: (AccountSetup) -> String?,
    onConnectJmap: (JmapSetup) -> String?,
    // Asks the core whether this JMAP server offers discoverable OAuth sign-in. Blocking, so the
    // caller runs it off-main; null (the default) means "never offer it", which keeps every
    // existing preview and test rendering the plain form.
    onCheckJmapSignIn: (suspend (String, String) -> Boolean)? = null,
    // Starts the JMAP browser sign-in for the typed email + server.
    onSignInJmap: (String, String) -> Unit = { _, _ -> },
    signingInJmap: Boolean = false,
    // Asks the typed mail server what it accepts. Null where there is no core to ask, which is a
    // preview or a test: the pane then simply never offers a sign-in.
    onCheckImapAuth: (suspend (uniffi.mailcal_bindings.ImapLoginRequest) -> uniffi.mailcal_bindings.ImapAuthOffer)? = null,
    onSignInImap: ((uniffi.mailcal_bindings.ImapLoginRequest) -> Unit)? = null,
    signingInImap: Boolean = false,
    initialKind: AccountKind = AccountKind.PASSWORD,
    // Which account types the picker shows, [AccountKind.offered] for what this build carries.
    // Passed in rather than read here so the form stays renderable without the core: every kind
    // by default, which is what a JVM test wants and what MainActivity overrides.
    offeredKinds: List<AccountKind> = AccountKind.entries,
    prefillEmail: String = "",
    prefillImapHost: String = "",
    prefillSmtpHost: String = "",
    prefillJmapServer: String = "",
    note: String? = null,
) {
    var kind by remember { mutableStateOf(initialKind) }
    var imapHost by remember { mutableStateOf(prefillImapHost) }
    var username by remember { mutableStateOf(prefillEmail) }
    var password by remember { mutableStateOf("") }
    var smtpHost by remember { mutableStateOf(prefillSmtpHost) }
    var caldavBaseUrl by remember { mutableStateOf("") }
    // JMAP reuses the shared username/password state (only one kind is active at a time), the
    // secret is one field, whether the server issued a password or an API token.
    var jmapServer by remember { mutableStateOf(prefillJmapServer) }
    // Whether this JMAP server advertises OAuth sign-in. Starts false and only ever becomes true
    // after the core says so, so we never show a button that dead-ends. Re-probed (debounced) as
    // the address/server is typed.
    var jmapSignInOffered by remember { mutableStateOf(false) }
    // Gates the Google sign-in button: the user must confirm they've signed up for Early Access
    // before we open the browser (Google hard-blocks anyone not on the allow-list).
    var googleEarlyAccessConfirmed by remember { mutableStateOf(false) }
    // What the typed mail server said it accepts.
    var imapAuth by remember { mutableStateOf<ImapAuthState>(ImapAuthState.Password) }
    var error by remember { mutableStateOf<String?>(null) }
    val ctx = LocalContext.current

    val canConnect = imapHost.isNotBlank() && username.isNotBlank() && password.isNotBlank()
    // JMAP needs an email and the secret; the server may be left blank.
    val canConnectJmap = username.isNotBlank() && password.isNotBlank()

    // Probe whether this server offers OAuth sign-in, debounced so it runs when typing settles
    // rather than on every keystroke, the check is several network round trips. Keyed on the JMAP
    // tab plus the two fields that identify the server, so switching tabs or editing the address
    // re-probes and a stale "offered" can never survive a change of server.
    if (onCheckJmapSignIn != null) {
        androidx.compose.runtime.LaunchedEffect(kind, username, jmapServer) {
            jmapSignInOffered = false
            if (kind != AccountKind.JMAP || !username.contains('@')) {
                return@LaunchedEffect
            }
            kotlinx.coroutines.delay(JMAP_SIGNIN_PROBE_DEBOUNCE_MS)
            jmapSignInOffered = onCheckJmapSignIn(username, jmapServer)
        }
    }

    // The same question of the typed mail server, debounced the same way. This pane's password
    // field stays on screen throughout (it is already there, and taking it away would erase a
    // password somebody is in the middle of typing), so an answer can only ever *add* the button
    // or a line of explanation.
    if (onCheckImapAuth != null) {
        androidx.compose.runtime.LaunchedEffect(kind, username, imapHost) {
            imapAuth = ImapAuthState.Password
            if (kind != AccountKind.PASSWORD || !username.contains('@') || imapHost.isBlank()) {
                return@LaunchedEffect
            }
            kotlinx.coroutines.delay(JMAP_SIGNIN_PROBE_DEBOUNCE_MS)
            imapAuth = ImapAuthState.of(
                onCheckImapAuth(
                    typedImapLoginRequest(username, imapHost, smtpHost, caldavBaseUrl)
                )
            )
        }
    }

    Column(
        modifier = Modifier
            .fillMaxSize()
            // The setup form is top-aligned and not inside a Scaffold, so it must clear the
            // system bars itself (the app is edge-to-edge); without this the title sits under
            // the clock and the Connect button under the navigation bar on a real device.
            .systemBarsPadding()
            .verticalScroll(rememberScrollState())
            .padding(24.dp),
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Text(text = L10n.setup_title(ctx), style = MaterialTheme.typography.headlineSmall)

        // Shown when the email-first flow routed here (nothing found, offline, an
        // unsupported provider) so the user knows why they're entering settings by hand.
        if (note != null) {
            Text(
                text = note,
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }

        SingleChoiceSegmentedButtonRow(modifier = Modifier.fillMaxWidth()) {
            offeredKinds.forEachIndexed { index, offeredKind ->
                SegmentedButton(
                    selected = kind == offeredKind,
                    onClick = { kind = offeredKind },
                    shape = SegmentedButtonDefaults.itemShape(
                        index = index,
                        count = offeredKinds.size,
                    ),
                ) { Text(offeredKind.label(ctx)) }
            }
        }

        when (kind) {
            AccountKind.MICROSOFT -> Text(
                text = L10n.setup_microsoft_note(ctx),
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            AccountKind.GOOGLE -> {
                Text(
                    text = L10n.setup_google_note(ctx),
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                GoogleEarlyAccessInfo(googleEarlyAccessConfirmed) { googleEarlyAccessConfirmed = it }
            }
            AccountKind.JMAP -> {
                Text(
                    text = L10n.setup_jmap_note(ctx),
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                SetupField(username, { username = it }, L10n.setup_field_email(ctx), keyboardType = KeyboardType.Email)
                // Offered above the secret field, but never instead of it: discovery can fail on
                // any server, so the manual path stays present and usable at all times.
                JmapSignInOffer(
                    offered = jmapSignInOffered,
                    signingIn = signingInJmap,
                    onClick = { onSignInJmap(username, jmapServer) },
                )
                PasswordField(password, { password = it }, L10n.setup_jmap_secret_placeholder(ctx))
                SetupField(jmapServer, { jmapServer = it }, L10n.setup_jmap_server_placeholder(ctx))
                Text(
                    text = L10n.setup_jmap_secret_note(ctx),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
            AccountKind.PASSWORD -> {
                Text(
                    text = L10n.setup_description(ctx),
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                SetupField(imapHost, { imapHost = it }, L10n.setup_field_mail_server(ctx), L10n.setup_hint_imap(ctx))
                SetupField(username, { username = it }, L10n.setup_field_email(ctx), keyboardType = KeyboardType.Email)
                if (imapAuth.offersSignIn || imapAuth.explainsRegistration) {
                    ImapAuthExplanation(imapAuth)
                }
                // Offered above the password field, but never instead of it: the manual pane is
                // where somebody lands when nothing was detected, and a server that declines
                // sign-in must still be connectable from here.
                if (imapAuth.offersSignIn && onSignInImap != null) {
                    SignInButton(
                        enabled = !signingInImap,
                        signingIn = signingInImap,
                        label = L10n.setup_imap_signin_button(ctx),
                    ) {
                        onSignInImap(
                            typedImapLoginRequest(username, imapHost, smtpHost, caldavBaseUrl)
                        )
                    }
                }
                PasswordField(password, { password = it }, L10n.setup_field_password(ctx))
                SetupField(smtpHost, { smtpHost = it }, L10n.setup_field_smtp_optional(ctx), L10n.setup_hint_smtp(ctx))
                SetupField(caldavBaseUrl, { caldavBaseUrl = it }, L10n.setup_field_caldav_optional(ctx))
            }
        }

        val message = error ?: externalError
        if (message != null) {
            Text(
                text = message,
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.error,
            )
        }

        when (kind) {
            AccountKind.MICROSOFT -> Button(
                onClick = { onSignInMicrosoft(username.ifBlank { null }) },
                enabled = !signingIn,
                modifier = Modifier.fillMaxWidth(),
            ) {
                if (signingIn) {
                    CircularProgressIndicator(modifier = Modifier.size(18.dp))
                } else {
                    Text(L10n.setup_microsoft_signin(ctx))
                }
            }
            // Disabled until the Early Access checkbox is confirmed (and never while a sign-in is
            // already running).
            AccountKind.GOOGLE -> GoogleSignInButton(
                enabled = googleEarlyAccessConfirmed && !signingInGoogle,
                signingIn = signingInGoogle,
                onClick = { onSignInGoogle(username.ifBlank { null }) },
            )
            AccountKind.JMAP -> ConnectButton(
                enabled = canConnectJmap && !connecting,
                connecting = connecting,
                label = L10n.action_connect(ctx),
                onClick = {
                    error = onConnectJmap(
                        JmapSetup(
                            email = username,
                            serverUrl = jmapServer.ifBlank { null },
                            password = password,
                        ),
                    )
                },
            )
            AccountKind.PASSWORD -> ConnectButton(
                enabled = canConnect && !connecting,
                connecting = connecting,
                label = L10n.action_connect(ctx),
                onClick = {
                    error = onConnect(
                        AccountSetup(
                            imapHost = imapHost,
                            username = username,
                            password = password,
                            smtpHost = smtpHost.ifBlank { null },
                            caldavBaseUrl = caldavBaseUrl.ifBlank { null },
                        ),
                    )
                },
            )
        }
        // Only when adding another account: back out to the running app.
        if (onCancel != null) {
            TextButton(onClick = onCancel, modifier = Modifier.fillMaxWidth()) {
                Text(L10n.action_cancel(ctx))
            }
        }
    }
}
