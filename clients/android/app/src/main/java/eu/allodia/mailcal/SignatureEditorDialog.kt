// The signature body editor (Settings → Signatures): the shared `clients/composer/dist/editor.html`
// bundle hosted body-only, with the same hardened WebView configuration as the composer
// (EditorWebView.kt, docs/composer-security.md). Authoring a signature is authoring mail content,
// so it gets the composer's gates, not a lighter set.
//
// The one thing it does that the composer does not is insert an image as a self-contained `data:`
// URI. That is what a signature stores (one file, no side-car blobs to lose) and what the core
// rewrites to a `cid:` part on send, because Outlook's reader blocks `data:` images.
package eu.allodia.mailcal

import android.os.Handler
import android.os.Looper
import android.webkit.WebView
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.unit.dp
import androidx.compose.ui.viewinterop.AndroidView
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import kotlin.concurrent.thread
import org.json.JSONObject
import uniffi.mailcal_bindings.ControlState
import uniffi.mailcal_bindings.SignatureIntent
import uniffi.mailcal_bindings.SignatureResult

internal const val SIGNATURE_SAVE_TAG = "signature-save"
internal const val SIGNATURE_IMAGE_TAG = "signature-image"
internal const val SIGNATURE_NAME_TAG = "signature-name"

// The editor for one signature: its name, the rich body, and an "add image" button. Save sends a
// `SignatureIntent.Save` under [id] (the entry's, or the page's `newId` for a new signature); the
// page's next outcome for [id] newer than [openedSerial] says what it came to, once: saved,
// deleted meanwhile or not written closes the editor, and refused keeps it open and says why.
//
// A full-screen Dialog, like the composer: the body is a live rich-text editor and a phone-sized
// alert box would leave it a few lines tall. The system back button reaches onDismissRequest, so
// back cancels exactly like the Close button.
@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun SignatureEditorDialog(
    id: String,
    title: String,
    initialName: String,
    // The stored HTML of an existing signature, or null for a new one.
    initialBodyHtml: String?,
    openedSerial: ULong,
    ui: SignaturesUi,
    onDismiss: () -> Unit,
) {
    val ctx = LocalContext.current
    val page = ui.page ?: return
    val html = remember(ctx) { loadComposerAsset(ctx) }
    val labelsJson = remember(ctx) { composerLabelsJson(ctx) }
    var name by remember { mutableStateOf(initialName) }
    var message by remember { mutableStateOf<String?>(null) }
    var seenSerial by remember { mutableStateOf(openedSerial) }
    var webView by remember { mutableStateOf<WebView?>(null) }
    val linkHost = rememberComposerLinkHost()
    val close by rememberUpdatedState(onDismiss)

    val outcome = page.outcome
    LaunchedEffect(outcome) {
        if (outcome == null || outcome.id != id || outcome.serial <= seenSerial) return@LaunchedEffect
        seenSerial = outcome.serial
        when (val result = outcome.result) {
            is SignatureResult.Refused -> message = signatureRefusalText(ctx, result.reason)
            SignatureResult.Saved, SignatureResult.Deleted, SignatureResult.NotWritten -> close()
        }
    }

    val imageRefusal by rememberUpdatedState(ui.imageRefusal)
    val pickImage = rememberLauncherForActivityResult(
        ActivityResultContracts.OpenDocument(),
    ) { uri ->
        if (uri == null) {
            return@rememberLauncherForActivityResult
        }
        message = null
        // Off the main thread: a content-resolver read of up to the core's limit is not work for the
        // frame that handled the tap.
        thread(name = "mailcal-signature-image") {
            val picked = readSignatureImage(ctx, uri, imageRefusal)
            Handler(Looper.getMainLooper()).post {
                when (picked) {
                    is SignatureImage.DataUrl ->
                        webView?.insertSignatureImage(picked.value, altText = picked.altText)
                    is SignatureImage.Refused -> message = signatureImageRefusalText(ctx, picked.refusal)
                    SignatureImage.Failed -> message = L10n.settings_signatures_image_failed(ctx)
                }
            }
        }
    }

    DisposableEffect(Unit) {
        onDispose {
            webView?.destroy()
            webView = null
        }
    }

    Dialog(
        onDismissRequest = onDismiss,
        properties = DialogProperties(
            usePlatformDefaultWidth = false,
            decorFitsSystemWindows = false,
        ),
    ) {
        SystemBarsMatchTheme()
        Surface(modifier = Modifier.fillMaxSize()) {
            Scaffold(
                modifier = Modifier.imePadding(),
                topBar = {
                    TopAppBar(
                        title = { Text(title) },
                        navigationIcon = {
                            IconButton(onClick = onDismiss) {
                                Icon(
                                    painter = painterResource(R.drawable.ic_close),
                                    contentDescription = L10n.action_cancel(ctx),
                                )
                            }
                        },
                        actions = {
                            // Opens the picker and starts nothing, so it is held with the page and
                            // never runs.
                            ControlIconButton(
                                icon = R.drawable.ic_image,
                                label = L10n.settings_signatures_insert_image(ctx),
                                control = page.save.copy(running = false),
                                tag = SIGNATURE_IMAGE_TAG,
                            ) { pickImage.launch(SIGNATURE_IMAGE_TYPES) }
                            ControlIconButton(
                                icon = R.drawable.ic_check,
                                label = L10n.settings_signatures_save(ctx),
                                control = ControlState(
                                    enabled = page.save.enabled && ui.nameRefusal(name) == null,
                                    running = page.save.running,
                                ),
                                tag = SIGNATURE_SAVE_TAG,
                            ) {
                                webView?.readSignatureBody { body, plain ->
                                    ui.onChange(SignatureIntent.Save(id, name, body, plain))
                                }
                            }
                        },
                    )
                },
            ) { padding ->
                Column(modifier = Modifier.fillMaxSize().padding(padding)) {
                    HeldFields(page.save) { enabled ->
                        OutlinedTextField(
                            value = name,
                            onValueChange = { name = it },
                            enabled = enabled,
                            modifier = Modifier
                                .fillMaxWidth()
                                .padding(horizontal = 16.dp)
                                .testTag(SIGNATURE_NAME_TAG),
                            singleLine = true,
                            label = { Text(L10n.settings_signatures_name_label(ctx)) },
                            placeholder = { Text(L10n.settings_signatures_name_placeholder(ctx)) },
                        )
                    }
                    message?.let { text ->
                        Text(
                            text = text,
                            color = MaterialTheme.colorScheme.error,
                            style = MaterialTheme.typography.bodySmall,
                            modifier = Modifier.padding(horizontal = 16.dp, vertical = 4.dp),
                        )
                    }
                    Box(modifier = Modifier.fillMaxSize().padding(top = 8.dp)) {
                        val dark = LocalAppDark.current
                        val currentDark = rememberUpdatedState(dark)
                        AndroidView(
                            modifier = Modifier.fillMaxSize(),
                            factory = { context ->
                                WebView(context).apply {
                                    installComposerHost(linkHost)
                                    configureSignatureWebView(
                                        labelsJson = labelsJson,
                                        bodyHtml = initialBodyHtml.orEmpty(),
                                        placeholder = L10n.settings_signatures_placeholder(ctx),
                                        dark = { currentDark.value },
                                    )
                                    loadDataWithBaseURL(
                                        "https://composer.local/",
                                        html,
                                        "text/html",
                                        "utf-8",
                                        null,
                                    )
                                    webView = this
                                }
                            },
                            update = { view -> view.evaluateJavascript(composerSchemeScript(dark), null) },
                        )
                    }
                    ComposerLinkDialogHost(linkHost, webView)
                }
            }
        }
    }
}

private fun WebView.configureSignatureWebView(
    labelsJson: String,
    bodyHtml: String,
    placeholder: String,
    dark: () -> Boolean,
) {
    applyEditorSecuritySettings()
    webViewClient = object : EditorWebViewClient() {
        // The bundle has parsed, so its window.* hooks exist, the same page-finished contract the
        // composer has. Order matters: `setComposerLabels` carries the composer's "Write your
        // message" placeholder, so the signature's own placeholder has to land after it.
        override fun onPageFinished(view: WebView?, url: String?) {
            // Android's WebView needs the same viewport fill the composer does: it is laid out
            // AFTER the page loads, so `100vh`/`100%` compute to 0 and the editor would collapse to
            // one tappable line (see fillViewport in editor.html).
            view?.evaluateJavascript("window.useNativeComposerChrome()", null)
            view?.evaluateJavascript("window.setComposerLabels($labelsJson)", null)
            view?.evaluateJavascript(composerSchemeScript(dark()), null)
            view?.announceComposerHost()
            view?.evaluateJavascript(
                "window.setSignatureBody(${JSONObject.quote(bodyHtml)}, ${JSONObject.quote(placeholder)})",
                null,
            )
            // Writing the signature is the only thing this screen is for, so the caret opens in it.
            // Asked for rather than assumed: the shared bundle focuses nothing of its own accord,
            // because in the composer the caret belongs in To (docs/contacts.md §4).
            view?.evaluateJavascript("window.focusComposerBody()", null)
        }
    }
}

// Reads back what the user authored: the HTML to store and its plain-text rendering. Silently does
// nothing if the bundle has not parsed yet, Save is then a no-op rather than storing an empty body
// over a signature the user was editing.
private fun WebView.readSignatureBody(onBody: (html: String, plain: String) -> Unit) {
    evaluateJavascript("window.signatureBody()") { encoded ->
        val json = decodeJsString(encoded) ?: return@evaluateJavascript
        val parsed = try {
            JSONObject(json)
        } catch (_: Exception) {
            return@evaluateJavascript
        }
        onBody(parsed.optString("body_html"), parsed.optString("body_plain"))
    }
}

private fun WebView.insertSignatureImage(dataUrl: String, altText: String) {
    val payload = JSONObject().put("data_url", dataUrl).put("alt_text", altText).toString()
    evaluateJavascript("window.insertSignatureImage(${JSONObject.quote(payload)})", null)
}
