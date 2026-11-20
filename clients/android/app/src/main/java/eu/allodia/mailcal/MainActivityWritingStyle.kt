// The writing assistant's records on the activity (docs/ai.md): read on connect and on the signals
// that move them, and the calls Settings → Writing style and Settings → Advanced make. Every read is
// from the core's memory, so the main thread may make it.
package eu.allodia.mailcal

import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.Surface
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import java.time.ZoneId
import uniffi.mailcal_bindings.AiIntent
import uniffi.mailcal_bindings.AiRemedy
import uniffi.mailcal_bindings.Intent
import uniffi.mailcal_bindings.MailcalApp
import uniffi.mailcal_bindings.StyleLearningIntent
import uniffi.mailcal_bindings.WritingStyleIntent
import uniffi.mailcal_bindings.writingStyleNameRefusal

/** Reads the page, the flow and the endpoint's form again. */
internal fun MainActivity.pullWritingStyle(app: MailcalApp) {
    writingStyle.page = app.writingStylePage()
    writingStyle.learning = app.writingStyleLearning()
    writingStyle.endpoint = app.aiEndpointForm()
    writingStyle.version += 1
    writingStyle.draftVersion += 1
}

/** Reads the endpoint's form again, and nothing else. */
internal fun MainActivity.pullAiEndpoint(app: MailcalApp) {
    writingStyle.endpoint = app.aiEndpointForm()
}

internal fun MainActivity.writingStyleUi(app: MailcalApp) = WritingStyleUi(
    page = writingStyle.page,
    learning = writingStyle.learning,
    version = writingStyle.version,
    reveal = { app.writingStyleReveal(it) },
    detail = { app.writingStyleDetail(it) },
    nameRefusal = ::writingStyleNameRefusal,
    onChange = { app.dispatch(Intent.Ai(AiIntent.WritingStyle(it))) },
    onSignIn = { changeAllodia(allodiaSignIn()) },
    zone = timeZone?.active?.let { runCatching { ZoneId.of(it) }.getOrNull() } ?: ZoneId.systemDefault(),
)

internal fun MainActivity.aiEndpointUi(app: MailcalApp) = AiEndpointUi(
    form = writingStyle.endpoint,
    gap = { address, model, declared -> app.aiEndpointGap(address, model, declared) },
    onChange = { app.dispatch(Intent.Ai(AiIntent.Endpoint(it))) },
)

/** Draft a reply for the composers this activity opens. */
internal fun MainActivity.composerReplyDraftUi(app: MailcalApp) = ComposerReplyDraftUi(
    read = { composition, from -> app.composerReplyDraft(composition, from) },
    report = { composition, message -> app.composerReplyDraftReport(composition, message) },
    onChange = { app.dispatch(Intent.Ai(AiIntent.ReplyDraft(it))) },
    onRemedy = { remedy -> openRemedy(app, remedy) },
    version = writingStyle.draftVersion,
    language = appLanguage(this),
)

// A remedy the composer cannot take in place (docs/ai.md rule 27). It leaves the composer as it is:
// the Allodia sign-in opens in the browser, and a place in Settings opens over the composer, whose
// Back returns to it. Learn style opens the learning flow there.
private fun MainActivity.openRemedy(app: MailcalApp, remedy: AiRemedy) {
    val category = when (remedyAction(remedy)) {
        RemedyAction.SIGN_IN -> return changeAllodia(allodiaSignIn())
        RemedyAction.OPEN_ENDPOINT -> SettingsCategory.ADVANCED
        RemedyAction.LEARN, RemedyAction.CHOOSE_STYLE -> SettingsCategory.WRITING_STYLE
        RemedyAction.MOVE_FLOW -> return
    }
    if (remedy == AiRemedy.LEARN_STYLE) {
        app.dispatch(Intent.Ai(AiIntent.WritingStyle(WritingStyleIntent.Learning(StyleLearningIntent.Open))))
    }
    settingsCategory = category
    settingsOverComposer = true
}

/** Settings over the composer, full screen as the composer is; leaving it shows the composer again. */
@Composable
internal fun MainActivity.SettingsOverComposer(instance: MailcalApp) {
    val leave = { settingsOverComposer = false }
    Dialog(
        onDismissRequest = leave,
        properties = DialogProperties(usePlatformDefaultWidth = false, decorFitsSystemWindows = false),
    ) {
        SystemBarsMatchTheme()
        Surface(modifier = Modifier.fillMaxSize()) {
            SettingsTabContent(instance, onLeave = leave)
        }
    }
}
