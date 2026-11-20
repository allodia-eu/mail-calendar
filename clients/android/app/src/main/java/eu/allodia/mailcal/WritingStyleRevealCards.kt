// Tone and approach as a card per topic, and the person's phrases as chips with what they avoid
// struck through (docs/ai.md rule 15). The heading and the text under it are the core's; this lays
// them out.
package eu.allodia.mailcal

import androidx.annotation.DrawableRes
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedCard
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.unit.dp
import uniffi.mailcal_bindings.WritingStyleCard
import uniffi.mailcal_bindings.WritingStylePhrases
import uniffi.mailcal_bindings.WritingStyleTopic

@DrawableRes
private fun topicIcon(topic: WritingStyleTopic): Int = when (topic) {
    WritingStyleTopic.REGISTER -> R.drawable.ic_tune
    WritingStyleTopic.STRUCTURE -> R.drawable.ic_reply
    WritingStyleTopic.MOVES -> R.drawable.ic_forum
    WritingStyleTopic.PUNCTUATION -> R.drawable.ic_text_fields
}

/** Tone and approach: one card per topic the core described. */
@Composable
internal fun StyleCardsView(cards: List<WritingStyleCard>, shown: Boolean) {
    Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
        cards.forEachIndexed { index, card -> StyleCard(card, Modifier.entrance(shown, Entrance.RISE, 60 + 90 * index)) }
    }
}

@Composable
private fun StyleCard(card: WritingStyleCard, modifier: Modifier) {
    val ctx = LocalContext.current
    val accent = MaterialTheme.colorScheme.primary
    OutlinedCard(modifier = modifier.fillMaxWidth().semantics(mergeDescendants = true) {}) {
        Column(modifier = Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                Box(
                    modifier = Modifier.size(28.dp).background(accent.copy(alpha = 0.1f), RoundedCornerShape(8.dp)),
                    contentAlignment = Alignment.Center,
                ) {
                    Icon(painterResource(topicIcon(card.topic)), contentDescription = null, tint = accent, modifier = Modifier.size(16.dp))
                }
                Text(
                    topicText(ctx, card.topic),
                    style = MaterialTheme.typography.labelLarge,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
            Text(card.heading, style = MaterialTheme.typography.titleMedium)
            if (card.body.isNotEmpty()) {
                Text(card.body, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
            }
        }
    }
}

/** The phrases the person uses, then those they avoid. */
@OptIn(ExperimentalLayoutApi::class)
@Composable
internal fun StylePhrasesView(phrases: WritingStylePhrases, shown: Boolean) {
    val ctx = LocalContext.current
    Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
        if (phrases.used.isNotEmpty()) {
            FlowRow(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.spacedBy(8.dp),
                verticalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                phrases.used.forEachIndexed { index, phrase ->
                    PhraseChip(phrase, avoided = false, Modifier.entrance(shown, Entrance.POP, 60 + 40 * index))
                }
            }
        }
        if (phrases.avoided.isNotEmpty()) {
            Text(
                L10n.reveal_avoid(ctx),
                style = MaterialTheme.typography.labelLarge,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(top = 10.dp).semantics { heading() },
            )
            FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                phrases.avoided.forEachIndexed { index, phrase ->
                    PhraseChip(phrase, avoided = true, Modifier.entrance(shown, Entrance.POP, 420 + 40 * index))
                }
            }
        }
    }
}

@Composable
private fun PhraseChip(phrase: String, avoided: Boolean, modifier: Modifier) {
    val colours = MaterialTheme.colorScheme
    val shape = RoundedCornerShape(8.dp)
    Text(
        phrase,
        style = MaterialTheme.typography.bodyMedium.copy(textDecoration = if (avoided) TextDecoration.LineThrough else null),
        color = if (avoided) colours.onSurfaceVariant else colours.onSecondaryContainer,
        modifier = modifier
            .heightIn(min = 32.dp)
            .background(if (avoided) colours.surface else colours.secondaryContainer, shape)
            .border(1.dp, if (avoided) colours.outlineVariant else colours.secondaryContainer, shape)
            .padding(horizontal = 14.dp, vertical = 6.dp),
    )
}
