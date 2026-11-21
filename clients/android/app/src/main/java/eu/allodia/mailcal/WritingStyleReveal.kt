// A new style shown back once (docs/ai.md rule 15), in the flow host: the pages the core composed,
// in a pager with an indicator, Back and Next moving between them and Save on the last. Which page
// is on screen, the language chosen and the name and notes as typed are all this keeps.
package eu.allodia.mailcal

import android.content.Context
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.pager.HorizontalPager
import androidx.compose.foundation.pager.PagerState
import androidx.compose.foundation.rememberScrollState
import androidx.compose.material3.FilterChip
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import java.time.ZoneId
import uniffi.mailcal_bindings.WritingStyleLanguage
import uniffi.mailcal_bindings.WritingStyleRefusal
import uniffi.mailcal_bindings.WritingStyleReveal
import uniffi.mailcal_bindings.WritingStyleRevealPage

internal const val REVEAL_NAME_TAG = "reveal-name"
internal const val REVEAL_NOTES_TAG = "reveal-notes"

internal fun revealPageTag(page: WritingStyleRevealPage) = "reveal-page-$page"

internal fun revealLanguageTag(code: String) = "reveal-language-$code"

/** What the reveal holds while it is on screen besides its page: the language, the name and the notes. */
internal class RevealFields(name: String, notes: String) {
    var name by mutableStateOf(name)
    var notes by mutableStateOf(notes)
    var language by mutableIntStateOf(0)

    // Why the last Save was refused, until the next.
    var refused by mutableStateOf<WritingStyleRefusal?>(null)
}

/**
 * The host's controls on the reveal's page [current]: Back on every page after the first, Next on
 * every page before the last and Save on it, held while the core refuses the name typed. Moving
 * between pages is the record's `paging`.
 */
internal fun revealChrome(
    ctx: Context,
    reveal: WritingStyleReveal,
    current: Int,
    name: String,
    nameRefusal: (String) -> WritingStyleRefusal?,
    onPage: (Int) -> Unit,
    onSave: () -> Unit,
    onClose: () -> Unit,
): FlowChrome {
    val count = reveal.pages.size
    val moving = reveal.paging
    val last = current >= count - 1
    return FlowChrome(
        title = L10n.settings_category_writing_style(ctx),
        close = FlowAction(L10n.action_close(ctx), reveal.close, onClose),
        back = if (current > 0) FlowAction(L10n.wizard_back(ctx), moving, { onPage(current - 1) }) else null,
        primary = if (last) {
            FlowAction(
                L10n.reveal_save(ctx),
                reveal.save.copy(enabled = reveal.save.enabled && nameRefusal(name) == null),
                onSave,
            )
        } else {
            FlowAction(L10n.wizard_next(ctx), moving, { onPage(current + 1) })
        },
        pages = FlowPages(count, current),
    )
}

@Composable
internal fun RevealPages(
    reveal: WritingStyleReveal,
    pager: PagerState,
    fields: RevealFields,
    nameRefusal: (String) -> WritingStyleRefusal?,
    zone: ZoneId,
    padding: PaddingValues,
) {
    val ctx = LocalContext.current
    HorizontalPager(
        state = pager,
        modifier = Modifier.fillMaxSize().padding(padding),
        userScrollEnabled = reveal.paging.enabled,
    ) { index ->
        val page = reveal.pages.getOrNull(index) ?: return@HorizontalPager
        val shown = rememberSeen(pager, index)
        val modifier = Modifier.testTag(revealPageTag(page))
        when (page) {
            WritingStyleRevealPage.READ -> FlowPage(
                revealPageTitle(ctx, page),
                modifier,
                large = true,
                bottom = {
                    Text(
                        L10n.reveal_pages_intro(ctx),
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier = Modifier.entrance(shown, Entrance.RISE, 900),
                    )
                },
            ) { StyleFactsView(reveal.facts, zone, shown) }
            WritingStyleRevealPage.NAME -> FlowPage(revealPageTitle(ctx, page), modifier) {
                RevealNameFields(reveal, fields, nameRefusal)
            }
            else -> FlowPage(revealPageTitle(ctx, page), modifier) {
                LanguageControl(reveal.languages, fields.language) { fields.language = it }
                val language = reveal.languages.getOrNull(fields.language)
                // Keyed by the language, so a page about another language plays in afresh.
                key(fields.language) { LanguageSection(page, language, shown) }
            }
        }
    }
}

// Whether page [index] has been settled on, so its pictures play once it is on screen rather than
// while it is still beside it.
@Composable
private fun rememberSeen(pager: PagerState, index: Int): Boolean {
    val reduced = rememberReducedMotion()
    var seen by remember { mutableStateOf(reduced) }
    LaunchedEffect(pager.settledPage) { if (pager.settledPage == index) seen = true }
    return seen
}

/** One language's section for a per-language page, or a line saying it has nothing for it. */
@Composable
internal fun LanguageSection(page: WritingStyleRevealPage, language: WritingStyleLanguage?, shown: Boolean) {
    val letter = language?.letter
    val habits = language?.habits
    val cards = language?.cards.orEmpty()
    val phrases = language?.phrases
    when {
        page == WritingStyleRevealPage.LETTER && letter != null -> StyleLetterView(letter, shown)
        page == WritingStyleRevealPage.HABITS && habits != null -> StyleHabitsView(habits, shown)
        page == WritingStyleRevealPage.VOICE && cards.isNotEmpty() -> StyleCardsView(cards, shown)
        page == WritingStyleRevealPage.PHRASES && phrases != null -> StylePhrasesView(phrases, shown)
        else -> NothingInLanguage()
    }
}

@Composable
private fun NothingInLanguage() {
    val ctx = LocalContext.current
    Text(
        L10n.reveal_nothing_in_language(ctx),
        style = MaterialTheme.typography.bodyLarge,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
    )
}

/** The language control, drawn only when there is more than one language to choose between. */
@Composable
internal fun LanguageControl(languages: List<WritingStyleLanguage>, selected: Int, onSelect: (Int) -> Unit) {
    if (languages.size < 2) return
    val ctx = LocalContext.current
    val locale = catalogLocale(LocalContext.current)
    val label = L10n.settings_language_heading(ctx)
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .horizontalScroll(rememberScrollState())
            .semantics { contentDescription = label },
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        languages.forEachIndexed { index, language ->
            FilterChip(
                selected = index == selected,
                onClick = { onSelect(index) },
                label = { Text(languageText(language.language, locale)) },
                modifier = Modifier.testTag(revealLanguageTag(language.language)),
            )
        }
    }
}

// The last page: the name the style goes by and the person's notes, which win over the
// description. Both are held while Save runs.
@Composable
private fun RevealNameFields(reveal: WritingStyleReveal, fields: RevealFields, nameRefusal: (String) -> WritingStyleRefusal?) {
    val ctx = LocalContext.current
    HeldFields(reveal.save) { enabled ->
        Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
            val refusal = nameRefusal(fields.name) ?: fields.refused
            OutlinedTextField(
                value = fields.name,
                onValueChange = {
                    fields.name = it
                    fields.refused = null
                },
                enabled = enabled,
                singleLine = true,
                isError = refusal != null,
                label = { Text(L10n.writing_style_name_label(ctx)) },
                supportingText = refusal?.let { { Text(writingStyleRefusalText(ctx, it)) } },
                modifier = Modifier.fillMaxWidth().testTag(REVEAL_NAME_TAG),
            )
            OutlinedTextField(
                value = fields.notes,
                onValueChange = { fields.notes = it },
                enabled = enabled,
                minLines = 4,
                label = { Text(L10n.writing_style_notes(ctx)) },
                supportingText = { Text(L10n.writing_style_notes_hint(ctx)) },
                modifier = Modifier.fillMaxWidth().testTag(REVEAL_NOTES_TAG),
            )
        }
    }
}
