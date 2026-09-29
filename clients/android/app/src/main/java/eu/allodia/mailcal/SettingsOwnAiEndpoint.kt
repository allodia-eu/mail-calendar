// Settings → Advanced → Own AI endpoint (docs/ai.md, "Where requests go"; docs/settings.md row 11):
// any server that speaks the OpenAI chat API, and where the person says it runs. On every platform,
// because it is how a build without Allodia's relay gets AI at all. The core validates the address
// and keeps the key in the platform keystore; this only collects the four answers.
package eu.allodia.mailcal

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.unit.dp
import uniffi.mailcal_bindings.JurisdictionClass
import uniffi.mailcal_bindings.OwnAiEndpoint
import uniffi.mailcal_bindings.OwnEndpointException

// The key field left empty keeps whatever key is stored, so an edit to the address or the model
// never asks for the key again; the core reads `null` as exactly that.
internal fun ownEndpointKeyArgument(typed: String): String? = typed.ifEmpty { null }

@Composable
internal fun OwnAiEndpointCard(
    endpoint: OwnAiEndpoint?,
    onSave: (baseUrl: String, model: String, declared: JurisdictionClass?, key: String?) -> OwnEndpointException?,
    onRemove: () -> OwnEndpointException?,
) {
    val ctx = LocalContext.current
    // Keyed on the stored settings, so a save or a removal shows what the core now holds.
    var address by remember(endpoint) { mutableStateOf(endpoint?.baseUrl.orEmpty()) }
    var key by remember(endpoint) { mutableStateOf("") }
    var model by remember(endpoint) { mutableStateOf(endpoint?.model.orEmpty()) }
    var declared by remember(endpoint) { mutableStateOf(endpoint?.declared) }
    var error by remember { mutableStateOf<String?>(null) }
    val where = listOf(
        JurisdictionClass.EU_NATIVE to L10n.ai_endpoint_where_eu_native(ctx),
        JurisdictionClass.EU_HOSTED to L10n.ai_endpoint_where_eu_hosted(ctx),
        JurisdictionClass.NON_EU to L10n.ai_endpoint_where_non_eu(ctx),
    )
    SettingsGroupCard(L10n.ai_endpoint_title(ctx), L10n.ai_endpoint_intro(ctx)) {
        Column(modifier = Modifier.fillMaxWidth()) {
            OutlinedTextField(
                value = address,
                onValueChange = { address = it },
                modifier = Modifier.fillMaxWidth(),
                singleLine = true,
                label = { Text(L10n.ai_endpoint_address(ctx)) },
                supportingText = { Text(L10n.ai_endpoint_address_hint(ctx)) },
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Uri),
            )
            OutlinedTextField(
                value = key,
                onValueChange = { key = it },
                modifier = Modifier.fillMaxWidth(),
                singleLine = true,
                label = { Text(L10n.ai_endpoint_key(ctx)) },
                supportingText = {
                    Text(
                        if (endpoint?.hasKey == true) {
                            L10n.ai_endpoint_key_stored(ctx)
                        } else {
                            L10n.ai_endpoint_key_hint(ctx)
                        },
                    )
                },
                visualTransformation = PasswordVisualTransformation(),
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Password),
            )
            OutlinedTextField(
                value = model,
                onValueChange = { model = it },
                modifier = Modifier.fillMaxWidth(),
                singleLine = true,
                label = { Text(L10n.ai_endpoint_model(ctx)) },
            )
            Text(
                L10n.ai_endpoint_where(ctx),
                style = MaterialTheme.typography.bodyLarge,
                modifier = Modifier.padding(top = 8.dp),
            )
            where.forEach { (place, label) ->
                StrategyRow(label = label, selected = declared == place) { declared = place }
            }
            error?.let {
                Text(it, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.error)
            }
            Row {
                TextButton(
                    onClick = {
                        val refused = onSave(address.trim(), model.trim(), declared, ownEndpointKeyArgument(key))
                        error = refused?.let { ownEndpointErrorText(ctx, it) }
                    },
                ) {
                    Text(L10n.ai_endpoint_save(ctx))
                }
                if (endpoint != null) {
                    TextButton(onClick = { error = onRemove()?.let { ownEndpointErrorText(ctx, it) } }) {
                        Text(L10n.ai_endpoint_remove(ctx))
                    }
                }
            }
        }
    }
}
