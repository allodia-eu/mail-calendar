// The signature body editor (Settings ▸ Signatures): the shared `clients/composer/dist/editor.html`
// bundle hosted body-only, with the same hardened WebView configuration as the composer
// (EditorWebView.kt, docs/composer-security.md). Authoring a signature is authoring mail content,
// so it gets the composer's gates, not a lighter set.
//
// The one thing it does that the composer does not is insert an image as a self-contained `data:`
// URI. That is what a signature stores (one file, no side-car blobs to lose) and what the core
// rewrites to a `cid:` part on send, because Outlook's reader blocks `data:` images.
package eu.allodia.mailcal

import android.content.Context
import android.net.Uri
import android.os.Handler
import android.os.Looper
import android.text.format.Formatter
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
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.unit.dp
import androidx.compose.ui.viewinterop.AndroidView
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import kotlin.concurrent.thread
import org.json.JSONObject

// The editor for one signature: its name, the rich body, and an "add image" button. `onSave`
// receives the name and both body renderings; the caller decides whether that is a create or an
// update (it knows which signature it opened).
//
// A full-screen Dialog, like the composer: the body is a live rich-text editor and a phone-sized
// alert box would leave it a few lines tall. The system back button reaches onDismissRequest, so
// back cancels exactly like the Close button.
@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun SignatureEditorDialog(
    title: String,
    initialName: String,
    // The stored HTML of an existing signature, or null for a new one.
    initialBodyHtml: String?,
    onSave: (name: String, bodyHtml: String, bodyPlain: String) -> Unit,
    onDismiss: () -> Unit,
) {
    val ctx = LocalContext.current
    val html = remember(ctx) { loadComposerAsset(ctx) }
    val labelsJson = remember(ctx) { composerLabelsJson(ctx) }
    var name by remember { mutableStateOf(initialName) }
    var imageError by remember { mutableStateOf<String?>(null) }
    var webView by remember { mutableStateOf<WebView?>(null) }

    val pickImage = rememberLauncherForActivityResult(
        ActivityResultContracts.OpenDocument(),
    ) { uri ->
        if (uri == null) {
            return@rememberLauncherForActivityResult
        }
        imageError = null
        // Read + base64-encode off the main thread: it is a content-resolver read of up to the cap,
        // which is not work for the frame that handled the tap.
        thread(name = "mailcal-signature-image") {
            val outcome = readSignatureImage(ctx, uri)
            Handler(Looper.getMainLooper()).post {
                when (outcome) {
                    is SignatureImage.DataUrl ->
                        webView?.insertSignatureImage(outcome.value, altText = outcome.altText)
                    is SignatureImage.TooLarge -> imageError = L10n.settings_signatures_image_too_large(
                        ctx,
                        limit = Formatter.formatShortFileSize(ctx, outcome.limitBytes.toLong()),
                    )
                    SignatureImage.Failed -> imageError = L10n.settings_signatures_image_failed(ctx)
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
                            IconButton(onClick = { pickImage.launch(arrayOf("image/*")) }) {
                                Icon(
                                    painter = painterResource(R.drawable.ic_image),
                                    contentDescription = L10n.settings_signatures_insert_image(ctx),
                                )
                            }
                            // A signature with no name is a row the user cannot tell apart in the
                            // picker, so Save waits for one.
                            IconButton(
                                enabled = name.isNotBlank(),
                                onClick = {
                                    webView?.readSignatureBody { body, plain ->
                                        onSave(name.trim(), body, plain)
                                    }
                                },
                            ) {
                                Icon(
                                    painter = painterResource(R.drawable.ic_check),
                                    contentDescription = L10n.settings_signatures_save(ctx),
                                )
                            }
                        },
                    )
                },
            ) { padding ->
                Column(modifier = Modifier.fillMaxSize().padding(padding)) {
                    OutlinedTextField(
                        value = name,
                        onValueChange = { name = it },
                        modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp),
                        singleLine = true,
                        label = { Text(L10n.settings_signatures_name_label(ctx)) },
                        placeholder = { Text(L10n.settings_signatures_name_placeholder(ctx)) },
                    )
                    imageError?.let { message ->
                        Text(
                            text = message,
                            color = MaterialTheme.colorScheme.error,
                            style = MaterialTheme.typography.bodySmall,
                            modifier = Modifier.padding(horizontal = 16.dp, vertical = 4.dp),
                        )
                    }
                    Box(modifier = Modifier.fillMaxSize().padding(top = 8.dp)) {
                        AndroidView(
                            modifier = Modifier.fillMaxSize(),
                            factory = { context ->
                                WebView(context).apply {
                                    configureSignatureWebView(
                                        labelsJson = labelsJson,
                                        bodyHtml = initialBodyHtml.orEmpty(),
                                        placeholder = L10n.settings_signatures_placeholder(ctx),
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
                        )
                    }
                }
            }
        }
    }
}

private fun WebView.configureSignatureWebView(
    labelsJson: String,
    bodyHtml: String,
    placeholder: String,
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

// Reads a picked image into a `data:` URI, or says why it can't. The size check is separate from the
// read failure so the user is told WHICH problem it is.
internal sealed interface SignatureImage {
    data class DataUrl(val value: String, val altText: String) : SignatureImage

    data class TooLarge(val limitBytes: Int) : SignatureImage

    data object Failed : SignatureImage
}

// The cap on an embedded signature image. A signature rides in EVERY message the account sends, so
// a 5 MB logo is 5 MB per mail, and base64 adds a third on top. 512 KB is generous for a logo and
// small enough that nobody notices it on the wire. Enforced here, where the file is picked, so the
// user is told; the core does not police it.
internal const val SIGNATURE_IMAGE_LIMIT_BYTES: Int = 512 * 1024

// The pure half, so the cap and the media-type refusal are testable without a content resolver.
// Anything that is not an `image/*` is refused here rather than embedded: the editor would drop it
// anyway (it only accepts `data:image/`), and the picker is where the user can still be told.
internal fun signatureImageDataUrl(bytes: ByteArray, mediaType: String?, altText: String): SignatureImage {
    if (bytes.size > SIGNATURE_IMAGE_LIMIT_BYTES) {
        return SignatureImage.TooLarge(SIGNATURE_IMAGE_LIMIT_BYTES)
    }
    if (bytes.isEmpty() || mediaType == null || !mediaType.startsWith("image/")) {
        return SignatureImage.Failed
    }
    val encoded = java.util.Base64.getEncoder().encodeToString(bytes)
    return SignatureImage.DataUrl("data:$mediaType;base64,$encoded", altText)
}

private fun readSignatureImage(ctx: Context, uri: Uri): SignatureImage = try {
    // One byte past the cap is enough to reject: the user may pick a 4 GB file from a cloud
    // provider, and `readBytes()` would pull all of it into memory to then refuse it.
    val bytes = ctx.contentResolver.openInputStream(uri)?.use {
        it.readNBytes(SIGNATURE_IMAGE_LIMIT_BYTES + 1)
    }
    if (bytes == null) {
        SignatureImage.Failed
    } else {
        signatureImageDataUrl(
            bytes = bytes,
            mediaType = ctx.contentResolver.getType(uri),
            altText = uri.lastPathSegment?.substringAfterLast('/').orEmpty(),
        )
    }
} catch (_: Exception) {
    SignatureImage.Failed
}
