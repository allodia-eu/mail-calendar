// The quote-style card on the Settings screen (see SettingsScreen.kt): how a reply or forward
// quotes the original. Two named styles, each shown as a worked example rather than described in
// words, the names alone ("Indented", "Line + header") don't tell you what you'd get, and the
// preview does. Below them, an opt-in toggle that puts the same choice in every composer so a
// single message can deviate from the default without changing it.
//
// The example content comes from ComposerQuote.example, which builds it from the same catalog keys
// a real quote uses, so the preview can't drift from what the composer actually renders. The
// previews mirror the shared editor's CSS (clients/composer/dist/editor.html) and the Rust renderer
// (mailcal-composer): indented = a left border and an inset; line + header = a top rule and a
// labelled header block at full width.
package eu.allodia.mailcal

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.RadioButton
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import uniffi.mailcal_bindings.QuoteStyleKind

// The app-level default reply/forward quote style, plus the opt-in per-message override. Applies to
// every new reply; persisted in the shared core.
@Composable
internal fun QuoteStyleCard(
    quoteStyle: QuoteStyleKind,
    perMessage: Boolean,
    onSet: (QuoteStyleKind) -> Unit,
    onSetPerMessage: (Boolean) -> Unit,
) {
    val ctx = LocalContext.current
    val example = ComposerQuote.example(ctx)
    Column(modifier = Modifier.fillMaxWidth()) {
        QuoteStyleOption(
            label = L10n.quote_style_indented(ctx),
            description = L10n.quote_style_indented_description(ctx),
            selected = quoteStyle == QuoteStyleKind.INDENTED,
            onSelect = { onSet(QuoteStyleKind.INDENTED) },
            example = example,
            style = QuoteStyleKind.INDENTED,
        )
        Spacer(modifier = Modifier.height(8.dp))
        QuoteStyleOption(
            label = L10n.quote_style_line_header(ctx),
            description = L10n.quote_style_line_header_description(ctx),
            selected = quoteStyle == QuoteStyleKind.LINE_AND_HEADER,
            onSelect = { onSet(QuoteStyleKind.LINE_AND_HEADER) },
            example = example,
            style = QuoteStyleKind.LINE_AND_HEADER,
        )
        Spacer(modifier = Modifier.height(12.dp))
        PerMessageToggle(checked = perMessage, onCheckedChange = onSetPerMessage)
    }
}

// One style: the radio + its name, a plain-language description, and the live example below.
@Composable
private fun QuoteStyleOption(
    label: String,
    description: String,
    selected: Boolean,
    onSelect: () -> Unit,
    example: QuoteExample,
    style: QuoteStyleKind,
) {
    Column(
        modifier = Modifier
            .fillMaxWidth()
            .clickable(role = Role.RadioButton, onClick = onSelect)
            .padding(vertical = 4.dp),
    ) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            RadioButton(selected = selected, onClick = onSelect)
            Text(label, style = MaterialTheme.typography.bodyLarge)
        }
        Text(
            description,
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.padding(start = 48.dp, end = 8.dp, bottom = 8.dp),
        )
        QuoteStylePreview(
            style = style,
            example = example,
            modifier = Modifier.padding(start = 48.dp, end = 8.dp),
        )
    }
}

// The worked example. Deliberately not interactive and not a real editor, just enough of the
// shape (the indent and left rule, or the divider and labelled header block) to recognise at a
// glance which one you want.
@Composable
private fun QuoteStylePreview(
    style: QuoteStyleKind,
    example: QuoteExample,
    modifier: Modifier = Modifier,
) {
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val rule = MaterialTheme.colorScheme.outlineVariant
    Column(
        modifier = modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(8.dp))
            .background(MaterialTheme.colorScheme.surfaceVariant)
            .padding(10.dp),
    ) {
        if (style == QuoteStyleKind.INDENTED) {
            Text(
                example.line,
                style = MaterialTheme.typography.labelSmall,
                color = muted,
            )
            Spacer(modifier = Modifier.height(6.dp))
            // The left rule + inset that the indented style renders the original in.
            Row(modifier = Modifier.fillMaxWidth()) {
                Spacer(
                    modifier = Modifier
                        .width(2.dp)
                        .height(18.dp)
                        .background(rule),
                )
                Text(
                    example.body,
                    style = MaterialTheme.typography.labelSmall,
                    modifier = Modifier.padding(start = 8.dp),
                )
            }
        } else {
            // The divider the original is set off by, then the labelled header block at full width.
            Spacer(
                modifier = Modifier
                    .fillMaxWidth()
                    .height(1.dp)
                    .background(rule),
            )
            Spacer(modifier = Modifier.height(6.dp))
            example.headers.forEach { (label, value) ->
                Row {
                    Text(
                        "$label: ",
                        style = MaterialTheme.typography.labelSmall,
                        fontWeight = FontWeight.Bold,
                        color = muted,
                    )
                    Text(value, style = MaterialTheme.typography.labelSmall, color = muted)
                }
            }
            Spacer(modifier = Modifier.height(6.dp))
            Text(example.body, style = MaterialTheme.typography.labelSmall)
        }
    }
}

// The advanced opt-in: show the style picker in every composer. Off by default, so a reply just
// uses the default above and the composer stays uncluttered.
@Composable
private fun PerMessageToggle(checked: Boolean, onCheckedChange: (Boolean) -> Unit) {
    val ctx = LocalContext.current
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clickable(role = Role.Switch) { onCheckedChange(!checked) }
            .padding(vertical = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(modifier = Modifier.weight(1f).padding(end = 12.dp)) {
            Text(
                L10n.settings_quote_per_message_heading(ctx),
                style = MaterialTheme.typography.bodyLarge,
            )
            Text(
                L10n.settings_quote_per_message_description(ctx),
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
        Switch(checked = checked, onCheckedChange = onCheckedChange)
    }
}
