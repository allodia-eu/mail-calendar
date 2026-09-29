// The reveal's fourth and fifth pages: tone and approach as three cards, and the person's phrases
// as chips, with what they avoid struck through.
package eu.allodia.mailcal

import androidx.annotation.DrawableRes
import androidx.compose.animation.animateContentSize
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.tween
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
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.rotate
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import uniffi.mailcal_bindings.LanguageStyleRow

// Step 4: register, structure and how the person declines and chases, one card each.
@Composable
internal fun RevealVoicePage(style: LanguageStyleRow) {
    val ctx = LocalContext.current
    val shown = rememberShown()
    val cards = listOf(
        Triple(R.drawable.ic_tune, L10n.reveal_register(ctx), revealCardText(style.registerHeadline, style.register)),
        Triple(R.drawable.ic_reply, L10n.reveal_structure(ctx), revealCardText(style.structureHeadline, style.structure)),
        Triple(R.drawable.ic_forum, L10n.reveal_moves(ctx), revealCardText(style.movesHeadline, style.moves)),
    ).filter { (_, _, text) -> text.headline.isNotEmpty() || text.body.isNotEmpty() }
    WizardPage(title = L10n.reveal_step_voice(ctx)) {
        cards.forEachIndexed { index, (icon, label, text) ->
            RevealCard(icon, label, text, Modifier.wizardEntrance(shown, WizardEntrance.RISE, 60 + 90 * index))
        }
    }
}

// One card: an icon, what it is about, a heading, and two lines of the description that open to
// the rest.
@Composable
private fun RevealCard(@DrawableRes icon: Int, label: String, text: RevealCardText, modifier: Modifier) {
    val ctx = LocalContext.current
    val reduced = rememberReducedMotion()
    var expanded by remember { mutableStateOf(false) }
    var overflows by remember { mutableStateOf(false) }
    val accent = MaterialTheme.colorScheme.primary
    OutlinedCard(modifier = modifier.fillMaxWidth()) {
        Column(modifier = Modifier.padding(start = 16.dp, end = 8.dp, top = 12.dp, bottom = 14.dp)) {
            Row(modifier = Modifier.heightIn(min = 40.dp), verticalAlignment = Alignment.CenterVertically) {
                Box(
                    modifier = Modifier.size(28.dp).background(accent.copy(alpha = 0.1f), RoundedCornerShape(7.dp)),
                    contentAlignment = Alignment.Center,
                ) {
                    Icon(painterResource(icon), contentDescription = null, tint = accent, modifier = Modifier.size(16.dp))
                }
                Text(
                    label,
                    style = MaterialTheme.typography.labelLarge,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.weight(1f).padding(start = 10.dp),
                )
                if (expanded || overflows) {
                    val turn by animateFloatAsState(if (expanded) 180f else 0f, tween(300, easing = RISE), label = "chevron")
                    TextButton(onClick = { expanded = !expanded }) {
                        Text(if (expanded) L10n.reveal_card_less(ctx) else L10n.reveal_card_more(ctx))
                        Icon(
                            painterResource(R.drawable.ic_keyboard_arrow_down),
                            contentDescription = null,
                            modifier = Modifier.size(18.dp).rotate(turn),
                        )
                    }
                }
            }
            Column(modifier = Modifier.padding(end = 8.dp)) {
                if (text.headline.isNotEmpty()) {
                    Text(
                        text.headline,
                        style = MaterialTheme.typography.titleMedium,
                        modifier = Modifier.padding(top = 6.dp, bottom = 3.dp),
                    )
                }
                if (text.body.isNotEmpty()) {
                    Text(
                        text.body,
                        style = MaterialTheme.typography.bodyMedium,
                        maxLines = if (expanded) Int.MAX_VALUE else 2,
                        overflow = TextOverflow.Ellipsis,
                        onTextLayout = { if (!expanded) overflows = it.hasVisualOverflow },
                        modifier = if (reduced) Modifier else Modifier.animateContentSize(tween(300, easing = RISE)),
                    )
                }
            }
        }
    }
}

// Step 5: the person's phrases, and what they avoid.
@OptIn(ExperimentalLayoutApi::class)
@Composable
internal fun RevealPhrasesPage(style: LanguageStyleRow) {
    val ctx = LocalContext.current
    val shown = rememberShown()
    WizardPage(title = L10n.reveal_phrases(ctx)) {
        if (style.phrases.isNotEmpty()) {
            FlowRow(
                modifier = Modifier.fillMaxWidth().padding(top = 6.dp),
                horizontalArrangement = Arrangement.spacedBy(8.dp, Alignment.CenterHorizontally),
                verticalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                style.phrases.forEachIndexed { index, phrase ->
                    PhraseChip(phrase, avoided = false, Modifier.wizardEntrance(shown, WizardEntrance.POP, 60 + 40 * index))
                }
            }
        }
        if (style.avoid.isNotEmpty()) {
            Text(
                L10n.reveal_avoid(ctx),
                style = MaterialTheme.typography.labelLarge,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(top = 10.dp).semantics { heading() },
            )
            FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                style.avoid.forEachIndexed { index, phrase ->
                    PhraseChip(phrase, avoided = true, Modifier.wizardEntrance(shown, WizardEntrance.POP, 420 + 40 * index))
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
        style = MaterialTheme.typography.bodyMedium.copy(
            textDecoration = if (avoided) TextDecoration.LineThrough else null,
        ),
        color = if (avoided) colours.onSurfaceVariant else colours.onSurface,
        modifier = modifier
            .heightIn(min = 32.dp)
            .background(if (avoided) colours.surface else colours.surfaceContainerLow, shape)
            .border(1.dp, colours.outlineVariant, shape)
            .padding(horizontal = 14.dp, vertical = 6.dp),
    )
}
