// Settings → Advanced's own AI endpoint (docs/ai.md rules 6 and 7): four fields the core tidies and
// judges, Save and Remove as changes on the form's record. What a save would refuse is asked of the
// core as the person types and said under its field; Save is held while it names anything. The
// stored key is never drawn back: the field says one is kept, and leaving it empty keeps it.
package eu.allodia.mailcal

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.unit.dp
import uniffi.mailcal_bindings.AiEndpointForm
import uniffi.mailcal_bindings.AiEndpointIntent
import uniffi.mailcal_bindings.AiEndpointResult
import uniffi.mailcal_bindings.JurisdictionClass

internal const val AI_ENDPOINT_ADDRESS_TAG = "ai-endpoint-address"
internal const val AI_ENDPOINT_KEY_TAG = "ai-endpoint-key"
internal const val AI_ENDPOINT_MODEL_TAG = "ai-endpoint-model"
internal const val AI_ENDPOINT_SAVE_TAG = "ai-endpoint-save"
internal const val AI_ENDPOINT_REMOVE_TAG = "ai-endpoint-remove"

internal fun aiEndpointWhereTag(declared: JurisdictionClass) = "ai-endpoint-where-$declared"

// The three answers to "where does it run?", in the order the form offers them.
private val DECLARATIONS = listOf(JurisdictionClass.EU_NATIVE, JurisdictionClass.EU_HOSTED, JurisdictionClass.NON_EU)

/**
 * The fields as typed, over the fields the form was drawn with: a redraw replaces a field only
 * while the person has not changed it, so a save ending never draws over what was typed meanwhile.
 */
internal class AiEndpointFields(form: AiEndpointForm) {
    var drawn by mutableStateOf(form)
        private set
    var address by mutableStateOf(form.address)
    var model by mutableStateOf(form.model)
    var declared by mutableStateOf(form.declared)
    var key by mutableStateOf("")

    // Fields the person has changed, whose gaps are said; all of them once a save was refused.
    var touched by mutableStateOf(emptySet<String>())

    fun redraw(form: AiEndpointForm) {
        val before = drawn
        if (address == before.address) address = form.address
        if (model == before.model) model = form.model
        if (declared == before.declared) declared = form.declared
        drawn = form
    }

    fun touch(field: String) {
        touched = touched + field
    }
}

@Composable
internal fun AiEndpointCard(ui: AiEndpointUi) {
    val ctx = LocalContext.current
    val form = ui.form ?: return
    val fields = remember { AiEndpointFields(form) }
    LaunchedEffect(form) { fields.redraw(form) }
    val serials = remember { OutcomeSerials(form.outcome?.serial) }
    var keyRefused by remember { mutableStateOf(false) }
    val outcome = form.outcome
    LaunchedEffect(outcome) {
        if (outcome == null || !serials.fresh(outcome.serial)) return@LaunchedEffect
        keyRefused = outcome.result == AiEndpointResult.KeyNotStored
        when (outcome.result) {
            is AiEndpointResult.Refused -> fields.touched = setOf("address", "model", "where")
            AiEndpointResult.Saved, AiEndpointResult.Removed -> {
                fields.key = ""
                fields.touched = emptySet()
            }
            AiEndpointResult.KeyNotStored, AiEndpointResult.NotWritten -> Unit
        }
    }
    val gaps = ui.gap(fields.address, fields.model, fields.declared)

    SettingsGroupCard(L10n.ai_endpoint_title(ctx), L10n.ai_endpoint_intro(ctx)) {
        HeldFields(form.save) { enabled ->
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                val addressGap = gaps.address?.takeIf { "address" in fields.touched }
                OutlinedTextField(
                    value = fields.address,
                    onValueChange = {
                        fields.address = it
                        fields.touch("address")
                    },
                    enabled = enabled,
                    singleLine = true,
                    isError = addressGap != null,
                    label = { Text(L10n.ai_endpoint_address(ctx)) },
                    supportingText = {
                        Text(addressGap?.let { endpointAddressGapText(ctx, it) } ?: L10n.ai_endpoint_address_hint(ctx))
                    },
                    keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Uri),
                    modifier = Modifier.fillMaxWidth().testTag(AI_ENDPOINT_ADDRESS_TAG),
                )
                OutlinedTextField(
                    value = fields.key,
                    onValueChange = {
                        fields.key = it
                        keyRefused = false
                    },
                    enabled = enabled,
                    singleLine = true,
                    isError = keyRefused,
                    visualTransformation = PasswordVisualTransformation(),
                    keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Password),
                    label = { Text(L10n.ai_endpoint_key(ctx)) },
                    supportingText = {
                        Text(
                            when {
                                keyRefused -> L10n.ai_endpoint_key_not_stored(ctx)
                                form.keyKept -> L10n.ai_endpoint_key_stored(ctx)
                                else -> L10n.ai_endpoint_key_hint(ctx)
                            },
                        )
                    },
                    modifier = Modifier.fillMaxWidth().testTag(AI_ENDPOINT_KEY_TAG),
                )
                val modelGap = gaps.modelMissing && "model" in fields.touched
                OutlinedTextField(
                    value = fields.model,
                    onValueChange = {
                        fields.model = it
                        fields.touch("model")
                    },
                    enabled = enabled,
                    singleLine = true,
                    isError = modelGap,
                    label = { Text(L10n.ai_endpoint_model(ctx)) },
                    supportingText = if (modelGap) {
                        { Text(L10n.ai_endpoint_gap_model_missing(ctx)) }
                    } else {
                        null
                    },
                    modifier = Modifier.fillMaxWidth().testTag(AI_ENDPOINT_MODEL_TAG),
                )
                Text(L10n.ai_endpoint_where(ctx), style = MaterialTheme.typography.labelLarge)
                Column {
                    DECLARATIONS.forEach { declared ->
                        ChoiceRow(
                            label = declarationText(declared),
                            selected = fields.declared == declared,
                            enabled = enabled,
                            tag = aiEndpointWhereTag(declared),
                        ) {
                            fields.declared = declared
                            fields.touch("where")
                        }
                    }
                }
                gaps.declaration?.takeIf { "where" in fields.touched }?.let {
                    Text(
                        endpointDeclarationGapText(ctx, it),
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.error,
                    )
                }
            }
        }
        Row(
            modifier = Modifier.fillMaxWidth().padding(top = 12.dp),
            horizontalArrangement = Arrangement.spacedBy(8.dp, androidx.compose.ui.Alignment.End),
        ) {
            ControlButton(
                label = L10n.ai_endpoint_remove(ctx),
                control = form.remove,
                tag = AI_ENDPOINT_REMOVE_TAG,
                destructive = true,
                outlined = true,
            ) { ui.onChange(AiEndpointIntent.Remove) }
            ControlButton(
                label = L10n.ai_endpoint_save(ctx),
                control = form.save,
                tag = AI_ENDPOINT_SAVE_TAG,
                ready = gaps.none(),
            ) {
                ui.onChange(
                    AiEndpointIntent.Save(
                        address = fields.address,
                        model = fields.model,
                        declared = fields.declared,
                        key = fields.key.takeIf { it.isNotEmpty() },
                    ),
                )
            }
        }
    }
}

@Composable
private fun declarationText(declared: JurisdictionClass): String {
    val ctx = LocalContext.current
    return when (declared) {
        JurisdictionClass.EU_NATIVE -> L10n.ai_endpoint_where_eu_native(ctx)
        JurisdictionClass.EU_HOSTED -> L10n.ai_endpoint_where_eu_hosted(ctx)
        JurisdictionClass.NON_EU, JurisdictionClass.UNKNOWN -> L10n.ai_endpoint_where_non_eu(ctx)
    }
}
