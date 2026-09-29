// The learning sheet (docs/ai.md, "Learning"): which account, which sent mail, what the device
// found there and exactly what would be sent where, then the run with its progress and a way to
// stop it, one page each in the frame the reveal uses. The steps and every call they make are
// LearnFlow's (WritingStyleLearnFlow.kt); this only draws them.
package eu.allodia.mailcal

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import java.time.LocalDate
import java.time.ZoneId
import java.time.format.DateTimeFormatter
import java.time.format.FormatStyle
import uniffi.mailcal_bindings.AiRoute
import uniffi.mailcal_bindings.LearningProgress
import uniffi.mailcal_bindings.LearningStage
import uniffi.mailcal_bindings.OwnAiEndpoint
import uniffi.mailcal_bindings.WritingStyleFailure
import uniffi.mailcal_bindings.WritingStyleSnapshot

@Composable
internal fun WritingStyleLearnDialog(
    flow: LearnFlow,
    snapshot: WritingStyleSnapshot,
    ownEndpoint: OwnAiEndpoint?,
    zone: ZoneId,
    onLearned: (styleId: String) -> Unit,
    onClose: () -> Unit,
) {
    val ctx = LocalContext.current
    val step = flow.step
    LaunchedEffect(step) {
        if (step is LearnStep.Learned) onLearned(step.styleId)
    }
    val pages = remember { learnPages(snapshot.accounts) }
    val pager = remember { WizardPager(pages.size) }
    val page = pages[pager.index]
    val account = (step as? LearnStep.Consent)?.account
    val primary = when (page) {
        LearnPage.ACCOUNT, LearnPage.RANGE -> WizardPrimary.Next(enabled = account != null)
        // Nothing usable says so and offers no button.
        LearnPage.CONSENT -> if (flow.canLearn) {
            WizardPrimary.Action(L10n.learn_consent_confirm(ctx)) {
                flow.consent(L10n.writing_style_default_name(ctx), catalogLocale(ctx))
                pager.go(pages.lastIndex)
            }
        } else {
            WizardPrimary.None
        }
        LearnPage.PROGRESS -> when (step) {
            is LearnStep.Learning -> WizardPrimary.Action(L10n.learn_stop(ctx), prominent = false, run = flow::stop)
            is LearnStep.Failed -> WizardPrimary.Action(L10n.action_close(ctx), run = onClose)
            else -> WizardPrimary.None
        }
    }
    WizardSheet(
        pager = pager,
        onClose = onClose,
        showsBack = page != LearnPage.PROGRESS && !pager.isFirst,
        primary = primary,
    ) { index ->
        when (pages[index]) {
            LearnPage.ACCOUNT -> WizardPage(title = L10n.learn_account_title(ctx)) {
                Column {
                    snapshot.accounts.forEach {
                        StrategyRow(it.email, selected = it.accountId == account) { flow.chooseAccount(it.accountId) }
                    }
                }
            }
            LearnPage.RANGE -> WizardPage(title = L10n.learn_range_title(ctx)) { RangeChoice(step, flow, zone) }
            LearnPage.CONSENT -> WizardPage(title = L10n.writing_style_learn(ctx)) {
                (step as? LearnStep.Consent)?.let { ConsentReport(it, snapshot.route, ownEndpoint, zone) }
            }
            LearnPage.PROGRESS -> if (step is LearnStep.Failed) {
                WizardPage(title = L10n.learn_failed_title(ctx)) {
                    Text(
                        writingStyleFailureText(ctx, step.failure, snapshot.route),
                        style = MaterialTheme.typography.bodyLarge,
                        color = if (step.failure is WritingStyleFailure.Cancelled) {
                            MaterialTheme.colorScheme.onSurfaceVariant
                        } else {
                            MaterialTheme.colorScheme.error
                        },
                    )
                }
            } else {
                LearningProgressPage(snapshot.learning, finished = step is LearnStep.Learned)
            }
        }
    }
}

@Composable
private fun RangeChoice(step: LearnStep, flow: LearnFlow, zone: ZoneId) {
    val ctx = LocalContext.current
    val locale = LocalConfiguration.current.locales[0]
    val range = (step as? LearnStep.Consent)?.range ?: LearnRange.Everything
    Column {
        StrategyRow(L10n.learn_range_all(ctx), selected = range == LearnRange.Everything) {
            if (range != LearnRange.Everything) flow.chooseRange(LearnRange.Everything)
        }
        StrategyRow(L10n.learn_range_until(ctx), selected = range is LearnRange.Until) {
            if (range !is LearnRange.Until) flow.chooseRange(LearnRange.Until(LocalDate.now(zone)))
        }
    }
    Text(
        L10n.learn_range_until_hint(ctx),
        style = MaterialTheme.typography.bodySmall,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
    )
    if (range is LearnRange.Until) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Text(L10n.learn_range_until_label(ctx), style = MaterialTheme.typography.bodyMedium)
            TextButton(onClick = { pickDate(ctx, range.day) { flow.chooseRange(LearnRange.Until(it)) } }) {
                Text(range.day.format(DateTimeFormatter.ofLocalizedDate(FormatStyle.MEDIUM).withLocale(locale)))
            }
        }
    }
}

// What the device found in the range, and, when any of it says enough, what would be sent where.
@Composable
private fun ConsentReport(step: LearnStep.Consent, route: AiRoute?, ownEndpoint: OwnAiEndpoint?, zone: ZoneId) {
    val ctx = LocalContext.current
    val locale = LocalConfiguration.current.locales[0]
    when (val corpus = step.corpus) {
        CorpusState.Reading -> ReadingLine()
        is CorpusState.Failed -> Text(
            writingStyleFailureText(ctx, corpus.failure, route),
            color = MaterialTheme.colorScheme.error,
        )
        is CorpusState.Ready -> {
            Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
                corpusReportLines(ctx, corpus.report, zone, locale).forEach {
                    Text(it, style = MaterialTheme.typography.bodyLarge)
                }
            }
            if (corpus.report.usable == 0u) {
                Text(L10n.learn_report_nothing(ctx), style = MaterialTheme.typography.bodyLarge)
            } else {
                Column(modifier = Modifier.padding(top = 6.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
                    Text(
                        L10n.learn_consent_title(ctx),
                        style = MaterialTheme.typography.titleSmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier = Modifier.semantics { heading() },
                    )
                    Text(
                        if (route == AiRoute.RELAY) {
                            L10n.learn_consent_relay(ctx)
                        } else {
                            L10n.learn_consent_own(ctx, host = ownEndpoint?.baseUrl?.let(::endpointHost).orEmpty())
                        },
                        style = MaterialTheme.typography.bodyLarge,
                    )
                }
            }
        }
    }
}

@Composable
private fun ReadingLine() {
    val ctx = LocalContext.current
    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        CircularProgressIndicator(modifier = Modifier.size(20.dp), strokeWidth = 2.dp)
        Text(L10n.learn_reading(ctx), style = MaterialTheme.typography.bodyMedium)
    }
}

// The core's own progress, alone in the middle of the page: reading the Sent folder first, then
// one request per part, counted. A finished run shows the bar full while the sheet hands over to
// the reveal.
@Composable
private fun LearningProgressPage(progress: LearningProgress?, finished: Boolean) {
    val ctx = LocalContext.current
    Box(modifier = Modifier.fillMaxSize().padding(WIZARD_GUTTER), contentAlignment = Alignment.Center) {
        Column(
            modifier = Modifier.widthIn(max = 320.dp).fillMaxWidth(),
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            when {
                finished -> LinearProgressIndicator(progress = { 1f }, modifier = Modifier.fillMaxWidth())
                progress?.stage == LearningStage.LEARNING && progress.total > 0u -> {
                    LinearProgressIndicator(
                        progress = { progress.done.toFloat() / progress.total.toFloat() },
                        modifier = Modifier.fillMaxWidth(),
                    )
                    Text(
                        L10n.learn_progress(ctx, done = progress.done.toString(), total = progress.total.toString()),
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        textAlign = TextAlign.Center,
                    )
                }
                else -> ReadingLine()
            }
        }
    }
}
