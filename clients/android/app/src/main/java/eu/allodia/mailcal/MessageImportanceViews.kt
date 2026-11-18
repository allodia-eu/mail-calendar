package eu.allodia.mailcal

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import uniffi.mailcal_bindings.MessageImportance

// A message's importance on screen (docs/importance.md). Which importance a row or a message has
// is the core's answer; this only draws it.

/** The glyph and colour for [importance], or `null` for normal, which draws nothing. */
@Composable
private fun importanceGlyph(importance: MessageImportance): Pair<Int, Color>? = when (importance) {
    // Red, as the mark that asks for attention; low is quiet.
    MessageImportance.HIGH -> R.drawable.ic_priority_high to MaterialTheme.colorScheme.error
    MessageImportance.LOW -> R.drawable.ic_arrow_downward to MaterialTheme.colorScheme.onSurfaceVariant
    MessageImportance.NORMAL -> null
}

/** The list row's mark, drawn before the flag; nothing for a normal message. */
@Composable
internal fun ImportanceMark(importance: MessageImportance, size: Dp = 15.dp) {
    val ctx = LocalContext.current
    val (glyph, tint) = importanceGlyph(importance) ?: return
    Icon(
        painter = painterResource(glyph),
        contentDescription = when (importance) {
            MessageImportance.HIGH -> L10n.a11y_importance_high(ctx)
            else -> L10n.a11y_importance_low(ctx)
        },
        tint = tint,
        modifier = Modifier.padding(start = 6.dp).size(size),
    )
}

/** The banner above an open message the core asked for; nothing for `null`. */
@Composable
internal fun ImportanceBanner(importance: MessageImportance?) {
    val ctx = LocalContext.current
    val shown = importance ?: return
    val (glyph, tint) = importanceGlyph(shown) ?: return
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .background(MaterialTheme.colorScheme.surfaceVariant)
            .padding(horizontal = 16.dp, vertical = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        // The text says it; the glyph is decoration.
        Icon(
            painter = painterResource(glyph),
            contentDescription = null,
            tint = tint,
            modifier = Modifier.size(16.dp),
        )
        Text(
            text = when (shown) {
                MessageImportance.HIGH -> L10n.reading_importance_high(ctx)
                else -> L10n.reading_importance_low(ctx)
            },
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
    }
}
