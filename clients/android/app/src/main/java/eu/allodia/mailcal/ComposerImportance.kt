// The composer's importance picker (docs/importance.md): an entry in the app bar's overflow, beside
// Signature, that opens a menu of what the core offers.
//
// The importance is the core's, held with the composition: the menu reads it there, a pick writes it
// there, and every save and the send take it from there, so the composer sends nothing of its own.
package eu.allodia.mailcal

import android.util.Log
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.painterResource
import uniffi.mailcal_bindings.ComposerImportance
import uniffi.mailcal_bindings.MailcalApp
import uniffi.mailcal_bindings.MessageImportance

private const val TAG = "Mailcal"

/**
 * The core's calls on one composition's importance. Each answers with the picker as it now stands,
 * or null when the core refused, which leaves what is on screen as it was. An interface so the JVM
 * suite, which loads no native library, can stand one in.
 */
internal interface ComposerImportanceStore {
    fun current(): ComposerImportance?

    fun set(importance: MessageImportance): ComposerImportance?
}

/** The core's picker for `composition`. */
internal fun composerImportanceStore(instance: MailcalApp, composition: String): ComposerImportanceStore =
    object : ComposerImportanceStore {
        override fun current() = asked { instance.composerImportance(composition) }

        override fun set(importance: MessageImportance) =
            asked { instance.setComposerImportance(composition, importance) }
    }

private fun asked(call: () -> ComposerImportance): ComposerImportance? = try {
    call()
} catch (e: Exception) {
    Log.w(TAG, "composer importance: ${e.javaClass.simpleName}")
    null
}

/** What a composer's importance is called in its menu. */
internal fun importanceLabel(ctx: android.content.Context, importance: MessageImportance): String =
    when (importance) {
        MessageImportance.HIGH -> L10n.compose_importance_high(ctx)
        MessageImportance.NORMAL -> L10n.compose_importance_normal(ctx)
        MessageImportance.LOW -> L10n.compose_importance_low(ctx)
    }

/** The menu, hanging from the overflow button, of every importance the core offers. */
@Composable
internal fun ComposerImportanceMenu(
    picker: ComposerImportance,
    expanded: Boolean,
    onExpand: (Boolean) -> Unit,
    onSelect: (MessageImportance) -> Unit,
) {
    val ctx = LocalContext.current
    DropdownMenu(expanded = expanded, onDismissRequest = { onExpand(false) }) {
        picker.options.forEach { importance ->
            DropdownMenuItem(
                text = { Text(importanceLabel(ctx, importance)) },
                trailingIcon = {
                    if (importance == picker.selected) {
                        Icon(painter = painterResource(R.drawable.ic_check), contentDescription = null)
                    }
                },
                onClick = {
                    onExpand(false)
                    onSelect(importance)
                },
            )
        }
    }
}
