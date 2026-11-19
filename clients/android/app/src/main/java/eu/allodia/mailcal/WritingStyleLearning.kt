// The learning flow in the flow host (docs/ai.md rules 11 to 14): each stage the core's record
// names, and exactly the controls it gives. Which stage is next, whether a control is held and
// which remedy a failure offers are the record's; this words them and lays them out.
package eu.allodia.mailcal

import android.content.Context
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import java.time.ZoneId
import uniffi.mailcal_bindings.AiRemedy
import uniffi.mailcal_bindings.StyleLearnRange
import uniffi.mailcal_bindings.StyleLearningIntent
import uniffi.mailcal_bindings.StyleLearningReview
import uniffi.mailcal_bindings.StyleLearningStage
import uniffi.mailcal_bindings.WritingStyleLearning

internal const val LEARN_STAGE_TAG = "learn-stage"
internal const val LEARN_PROGRESS_TAG = "learn-progress"

internal fun learnAccountTag(id: String) = "learn-account-$id"

internal fun learnRangeTag(range: StyleLearnRange) = when (range) {
    StyleLearnRange.Everything -> "learn-range-everything"
    StyleLearnRange.UntilEndOfLastYear -> "learn-range-until-last-year"
    is StyleLearnRange.Between -> "learn-range-between"
}

/**
 * The host's controls for a stage, each from the record: Back, the one primary the stage has
 * (Next, Learn, Stop or the failure's remedy) and Close. [onRemedy] is told the remedy when it is
 * one the client opens rather than one the core moves the flow by.
 */
internal fun learningChrome(
    ctx: Context,
    learning: WritingStyleLearning,
    send: (StyleLearningIntent) -> Unit,
    onRemedy: (AiRemedy) -> Unit,
): FlowChrome {
    val stage = learning.stage
    val remedy = (stage as? StyleLearningStage.Failed)?.remedy
    val primary = learning.next?.let { FlowAction(L10n.wizard_next(ctx), it, { send(StyleLearningIntent.Next) }) }
        ?: learning.learn?.let {
            FlowAction(L10n.learn_consent_confirm(ctx), it, {
                send(StyleLearningIntent.Learn(L10n.writing_style_default_name(ctx), appLanguage(ctx)))
            })
        }
        ?: learning.stop?.let { FlowAction(L10n.learn_stop(ctx), it, { send(StyleLearningIntent.Stop) }, outlined = true) }
        ?: learning.remedy?.takeIf { remedy != null }?.let { control ->
            FlowAction(aiRemedyText(ctx, checkNotNull(remedy)), control, {
                if (remedyAction(remedy) == RemedyAction.MOVE_FLOW) send(StyleLearningIntent.Remedy) else onRemedy(remedy)
            })
        }
    return FlowChrome(
        title = L10n.settings_category_writing_style(ctx),
        close = FlowAction(L10n.action_close(ctx), learning.close, { send(StyleLearningIntent.Close) }),
        back = learning.back?.let { FlowAction(L10n.wizard_back(ctx), it, { send(StyleLearningIntent.Back) }) },
        primary = primary,
    )
}

/** The stage on screen, from `ChooseAccount` to `Failed`; `Learned` is the reveal's. */
@Composable
internal fun LearningStageView(stage: StyleLearningStage, zone: ZoneId, send: (StyleLearningIntent) -> Unit) {
    val ctx = LocalContext.current
    val locale = LocalConfiguration.current.locales[0]
    when (stage) {
        is StyleLearningStage.ChooseAccount -> FlowPage(L10n.learn_account_title(ctx), Modifier.testTag(LEARN_STAGE_TAG)) {
            Column {
                stage.accounts.forEach { account ->
                    ChoiceRow(account.email, selected = account.accountId == stage.selected, tag = learnAccountTag(account.accountId)) {
                        send(StyleLearningIntent.SelectAccount(account.accountId))
                    }
                }
            }
        }
        is StyleLearningStage.ChooseRange -> FlowPage(L10n.learn_range_title(ctx), Modifier.testTag(LEARN_STAGE_TAG)) {
            Supporting(stage.email)
            LearnRangeChoice(stage, send)
            Supporting(sentReachText(ctx, stage.reach, locale))
        }
        is StyleLearningStage.Review -> FlowPage(L10n.writing_style_learn(ctx), Modifier.testTag(LEARN_STAGE_TAG)) {
            Supporting(stage.email)
            ReviewView(stage, zone)
        }
        is StyleLearningStage.Learning -> LearningProgress(stage.done, stage.total)
        is StyleLearningStage.Failed -> FlowPage(L10n.learn_failed_title(ctx), Modifier.testTag(LEARN_STAGE_TAG)) {
            Text(aiFailureText(ctx, stage.failure), style = MaterialTheme.typography.bodyLarge)
        }
        is StyleLearningStage.Learned -> Unit
    }
}

@Composable
private fun Supporting(text: String) {
    Text(text, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
}

// What the core read on the device, then what Learn would send where. Nothing has left yet.
@Composable
private fun ReviewView(stage: StyleLearningStage.Review, zone: ZoneId) {
    val ctx = LocalContext.current
    val locale = LocalConfiguration.current.locales[0]
    val report = when (val review = stage.review) {
        StyleLearningReview.Reading -> {
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                CircularProgressIndicator(modifier = Modifier.size(20.dp), strokeWidth = 2.dp)
                Text(L10n.learn_reading(ctx), style = MaterialTheme.typography.bodyMedium)
            }
            null
        }
        is StyleLearningReview.Ready -> review.report
        is StyleLearningReview.NothingUsable -> review.report
    }
    if (report != null) {
        Card(
            modifier = Modifier.fillMaxWidth(),
            colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surfaceContainerHigh),
        ) {
            Column(modifier = Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                learnReportLines(ctx, report, zone, locale).forEach { Text(it, style = MaterialTheme.typography.bodyLarge) }
            }
        }
    }
    Supporting(sentReachText(ctx, stage.reach, locale))
    if (stage.review is StyleLearningReview.NothingUsable) {
        Text(L10n.learn_report_nothing(ctx), style = MaterialTheme.typography.bodyLarge)
    }
    stage.sendsTo?.let { destination ->
        Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
            Text(
                L10n.learn_consent_title(ctx),
                style = MaterialTheme.typography.titleSmall,
                modifier = Modifier.padding(top = 6.dp).semantics { heading() },
            )
            Text(destinationText(ctx, destination), style = MaterialTheme.typography.bodyLarge)
        }
    }
    stage.cannotSend?.let { Text(aiFailureText(ctx, it), color = MaterialTheme.colorScheme.error) }
    if (!stage.keywordKept) Supporting(L10n.learn_keyword_not_kept(ctx))
}

// The run's progress, alone in the middle: a count once the run knows its total.
@Composable
private fun LearningProgress(done: UInt, total: UInt) {
    val ctx = LocalContext.current
    val fraction = learnProgressFraction(done, total)
    Box(modifier = Modifier.fillMaxSize().padding(FLOW_GUTTER).testTag(LEARN_PROGRESS_TAG), contentAlignment = Alignment.Center) {
        Column(
            modifier = Modifier.widthIn(max = 360.dp).fillMaxWidth(),
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.spacedBy(16.dp),
        ) {
            if (fraction == null) {
                LinearProgressIndicator(modifier = Modifier.fillMaxWidth())
            } else {
                LinearProgressIndicator(progress = { fraction }, modifier = Modifier.fillMaxWidth())
            }
            Text(
                learnProgressText(ctx, done, total),
                style = MaterialTheme.typography.bodyLarge,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                textAlign = TextAlign.Center,
            )
        }
    }
}
