// The learning sheet (docs/ai.md, "Learning"): which account, which sent mail, what the device
// found there and exactly what would be sent where, then the run with its progress and a way to
// stop it. Full screen, as this platform's other settings flows are. The steps and every call they
// make are LearnFlow's (WritingStyleLearnFlow.kt); this only draws them.
package eu.allodia.mailcal

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import java.time.LocalDate
import java.time.ZoneId
import java.time.format.DateTimeFormatter
import java.time.format.FormatStyle
import uniffi.mailcal_bindings.AiRoute
import uniffi.mailcal_bindings.LearningProgress
import uniffi.mailcal_bindings.LearningStage
import uniffi.mailcal_bindings.OwnAiEndpoint
import uniffi.mailcal_bindings.WritingStyleSnapshot

@OptIn(ExperimentalMaterial3Api::class)
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
    Dialog(
        onDismissRequest = onClose,
        properties = DialogProperties(usePlatformDefaultWidth = false, decorFitsSystemWindows = false),
    ) {
        SystemBarsMatchTheme()
        Surface(modifier = Modifier.fillMaxSize()) {
            Scaffold(
                topBar = {
                    TopAppBar(
                        title = { Text(L10n.writing_style_learn(ctx)) },
                        navigationIcon = {
                            IconButton(onClick = onClose) {
                                Icon(
                                    painter = painterResource(R.drawable.ic_close),
                                    contentDescription = L10n.action_close(ctx),
                                )
                            }
                        },
                    )
                },
            ) { padding ->
                Column(
                    modifier = Modifier
                        .fillMaxSize()
                        .padding(padding)
                        .verticalScroll(rememberScrollState())
                        .padding(horizontal = 16.dp, vertical = 8.dp),
                    verticalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    when (step) {
                        is LearnStep.ChooseAccount -> {
                            Text(L10n.learn_account_title(ctx), style = MaterialTheme.typography.titleMedium)
                            step.accounts.forEach { account ->
                                Text(
                                    account.email,
                                    style = MaterialTheme.typography.bodyLarge,
                                    modifier = Modifier
                                        .fillMaxWidth()
                                        .clickable { flow.chooseAccount(account.accountId) }
                                        .padding(vertical = 12.dp),
                                )
                            }
                        }
                        is LearnStep.Consent -> ConsentStep(step, flow, snapshot.route, ownEndpoint, zone)
                        is LearnStep.Learning -> LearningStep(snapshot.learning, flow)
                        is LearnStep.Failed -> {
                            Text(L10n.learn_failed_title(ctx), style = MaterialTheme.typography.titleMedium)
                            Text(writingStyleFailureText(ctx, step.failure, snapshot.route))
                        }
                        // The caller opens the style in place of this sheet.
                        is LearnStep.Learned -> Unit
                    }
                }
            }
        }
    }
}

@Composable
private fun ConsentStep(
    step: LearnStep.Consent,
    flow: LearnFlow,
    route: AiRoute?,
    ownEndpoint: OwnAiEndpoint?,
    zone: ZoneId,
) {
    val ctx = LocalContext.current
    val locale = LocalConfiguration.current.locales[0]
    val range = step.range
    Text(L10n.learn_range_title(ctx), style = MaterialTheme.typography.titleMedium)
    StrategyRow(L10n.learn_range_all(ctx), selected = range == LearnRange.Everything) {
        if (range != LearnRange.Everything) flow.chooseRange(LearnRange.Everything)
    }
    StrategyRow(L10n.learn_range_until(ctx), selected = range is LearnRange.Until) {
        if (range !is LearnRange.Until) flow.chooseRange(LearnRange.Until(LocalDate.now(zone)))
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
    HorizontalDivider(modifier = Modifier.padding(vertical = 8.dp))
    when (val corpus = step.corpus) {
        CorpusState.Reading -> Row(
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            CircularProgressIndicator(modifier = Modifier.size(20.dp), strokeWidth = 2.dp)
            Text(L10n.learn_reading(ctx), style = MaterialTheme.typography.bodyMedium)
        }
        is CorpusState.Failed -> Text(
            writingStyleFailureText(ctx, corpus.failure, route),
            color = MaterialTheme.colorScheme.error,
        )
        is CorpusState.Ready -> {
            corpusReportLines(ctx, corpus.report, zone, locale).forEach {
                Text(it, style = MaterialTheme.typography.bodyMedium)
            }
            if (corpus.report.usable == 0u) {
                Text(L10n.learn_report_nothing(ctx), style = MaterialTheme.typography.bodyMedium)
            } else {
                Text(
                    L10n.learn_consent_title(ctx),
                    style = MaterialTheme.typography.titleMedium,
                    modifier = Modifier.padding(top = 8.dp),
                )
                Text(
                    if (route == AiRoute.RELAY) {
                        L10n.learn_consent_relay(ctx)
                    } else {
                        L10n.learn_consent_own(ctx, host = ownEndpoint?.baseUrl?.let(::endpointHost).orEmpty())
                    },
                    style = MaterialTheme.typography.bodyMedium,
                )
                Button(onClick = { flow.consent(L10n.writing_style_default_name(ctx), catalogLocale(ctx)) }) {
                    Text(L10n.learn_consent_confirm(ctx))
                }
            }
        }
    }
}

// The core's own progress: reading the Sent folder first, then one request per part, counted.
@Composable
private fun LearningStep(progress: LearningProgress?, flow: LearnFlow) {
    val ctx = LocalContext.current
    if (progress?.stage == LearningStage.LEARNING && progress.total > 0u) {
        Text(
            L10n.learn_progress(ctx, done = progress.done.toString(), total = progress.total.toString()),
            style = MaterialTheme.typography.bodyMedium,
        )
        LinearProgressIndicator(
            progress = { progress.done.toFloat() / progress.total.toFloat() },
            modifier = Modifier.fillMaxWidth(),
        )
    } else {
        Text(L10n.learn_reading(ctx), style = MaterialTheme.typography.bodyMedium)
        LinearProgressIndicator(modifier = Modifier.fillMaxWidth())
    }
    TextButton(onClick = flow::stop) { Text(L10n.learn_stop(ctx)) }
}
