// The rich compose / reply / forward surface. A FULL-SCREEN dialog (not the old AlertDialog, which
// only used part of the viewport): a top app bar with Close + Send, the address fields, and the
// editor filling everything that's left. The WebView loads only the bundled editor asset; Rust owns
// validation/rendering through submitRichMail.
//
// It stays a `Dialog` rather than a screen swapped into MainActivity's `when`, so the caller keeps
// owning "is the composer open" as per-row state (see MailRows/ReadingScreen) and the system back
// button routes to onDismissRequest, full-screen chrome without restructuring navigation.
package eu.allodia.mailcal

import android.webkit.WebView
import androidx.compose.foundation.background
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
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.SideEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.unit.dp
import androidx.compose.ui.viewinterop.AndroidView
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import androidx.compose.ui.window.DialogWindowProvider
import androidx.core.view.WindowCompat
import java.io.File
import org.json.JSONObject
import uniffi.mailcal_bindings.AccountRow
import uniffi.mailcal_bindings.ComposerFileAttachment
import uniffi.mailcal_bindings.QuoteStyleKind
import uniffi.mailcal_bindings.RecipientMatch
import uniffi.mailcal_bindings.Recipients

@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun RichComposeMessageDialog(
    mode: RichComposeMode,
    onDismiss: () -> Unit,
    // `from` is the account id the user picked in the From dropdown; the core sends as, and
    // through, that account. Null only when there are no configured accounts to pick from.
    onSubmitRich: (
        from: String?,
        recipients: Recipients,
        subject: String,
        documentJson: String,
        files: List<ComposerFileAttachment>,
    ) -> Boolean,
    // Every configured account, for the From dropdown, and the one it opens on: the account that
    // received the mail being replied to/forwarded, else the selected mailbox's account, else the
    // app-level default send account (all resolved by the caller).
    accounts: List<AccountRow>,
    initialFrom: String? = null,
    initialTo: String = "",
    initialCc: String = "",
    // Bcc and Subject are pre-fillable for the same reason To/Cc are: a mail link (`mailto:`)
    // may name them. Every field stays editable, and nothing is sent without the user's Send.
    initialBcc: String = "",
    initialSubject: String = "",
    // A plain-text body the composer opens with, a mail link's `body=`. Seeded into the editor
    // as text, never markup (docs/composer-security.md, Gate 12), with the caret left after it
    // so the user writes on from there.
    initialBody: String = "",
    // The quoted-original seed (a `Block::Quote`-shaped JSON) injected once the editor finishes
    // loading, or null for a new message / a reply with no body loaded; `quoteStyle` is the app
    // default the quote is seeded with.
    quote: String? = null,
    quoteStyle: QuoteStyleKind = QuoteStyleKind.INDENTED,
    // Whether the user has opted into choosing the quote style per message (Settings ▸ Composing).
    // Off by default, and then the composer shows no style picker at all, the quote just uses the
    // app default above.
    quoteStylePerMessage: Boolean = false,
    // Ranked recipient suggestions for a partially-typed address, answered by the core from synced
    // contacts and from people the user has written to. Null disables autosuggest (screenshot runs
    // and tests, which must not depend on a populated people index).
    suggestionsFor: ((String) -> List<RecipientMatch>)? = null,
    // The signature library + the two lookups the core answers, or null to leave signatures out of
    // this composer entirely (a screenshot run, a test).
    signatures: ComposerSignatures? = null,
) {
    val ctx = LocalContext.current
    val density = LocalDensity.current
    val html = remember(ctx) { loadComposerAsset(ctx) }
    // The editor's own chrome (placeholder, toolbar) localised from the l10n catalog and injected
    // once the bundle loads; the editor is a shared asset with English baked in, so each client
    // passes its own strings (see setComposerLabels in editor.html).
    val labelsJson = remember(ctx) { composerLabelsJson(ctx) }
    // The From account. Falls back to the first configured account when the caller's preferred id
    // names none (it was removed while the list was stale), the dropdown must never open blank.
    var from by remember(initialFrom, accounts) {
        mutableStateOf(accounts.firstOrNull { it.id == initialFrom } ?: accounts.firstOrNull())
    }
    // Every address the caller pre-filled is finished, nothing here is being typed, so the fields
    // open with all of them committed and each renders as its own pill (see `seededRecipientField`).
    // The normalised values are what the dirty check below compares against, too: compare against
    // the RAW ones and every reply would open already counting as an edited draft, and ask before
    // closing something nobody typed into.
    val seededTo = seededRecipientField(initialTo)
    val seededCc = seededRecipientField(initialCc)
    val seededBcc = seededRecipientField(initialBcc)
    var to by remember { mutableStateOf(seededTo) }
    var cc by remember { mutableStateOf(seededCc) }
    var bcc by remember { mutableStateOf(seededBcc) }
    var subject by remember { mutableStateOf(initialSubject) }
    // Cc/Bcc are tucked away by default (matching Gmail/Thunderbird) and revealed by the chevron on
    // the To row. Opened from the start when either arrives pre-filled, so those addresses are
    // never hidden behind a tap the user doesn't know to make.
    var showCcBcc by remember { mutableStateOf(revealsCcBcc(initialCc, initialBcc)) }
    var prepareError by remember { mutableStateOf(false) }
    var webView by remember { mutableStateOf<WebView?>(null) }
    // The single-scroll model: the WebView owns the one scroll (so its native caret-following and
    // drag-to-scroll just work), and the address-field header is a native overlay drawn on top of
    // it whose vertical offset tracks the WebView's scroll, so it scrolls away as the message
    // grows, exactly like Thunderbird. `headerHeightPx` is fed back to the editor as its top inset
    // so the text starts just below the overlay and the two move in lockstep.
    var scrollY by remember { mutableIntStateOf(0) }
    var headerHeightPx by remember { mutableIntStateOf(0) }
    var attachments by remember { mutableStateOf<List<PickedComposerFile>>(emptyList()) }
    // Pictures dropped on the composer, waiting on the one question they raise. Held rather than
    // acted on, because the answer decides whether they become body content or attachments.
    var droppedPictures by remember { mutableStateOf<List<PickedComposerFile>>(emptyList()) }
    val pickAttachments = rememberAttachmentPicker(
        onStaged = { attachments = attachments + it },
        onFailed = { prepareError = true },
    )
    // The per-message style override of the persisted default, re-styles the quoted original in
    // place without disturbing the user's typed message.
    var style by remember { mutableStateOf(quoteStyle) }
    // The user's explicit signature choice for this message, or null to follow the From account.
    var signatureChoice by remember { mutableStateOf<SignatureChoice?>(null) }
    var signatureMenuOpen by remember { mutableStateOf(false) }
    // The editor document as it stood once the quote and signature were seeded, the baseline the
    // discard guard compares against, so a reply that merely carries its quoted original does not
    // open already dirty. Null until the editor answers, which reads as "nothing to lose".
    var seedDocument by remember { mutableStateOf<String?>(null) }
    var confirmingDiscard by remember { mutableStateOf(false) }
    // Resolved fresh on every read rather than held in state: the account's assignment can change
    // under an open composer (Settings is reachable from the notification shade), and this is a
    // cheap in-memory core lookup.
    val signature = signatures?.resolve(signatureChoice, from?.id, mode)
    // The factory below runs once, but the editor finishes parsing later, and `from` can still
    // settle in between (the account list arrives after the composer opened). Hold the latest
    // resolution so the page-finished seed is the current one, not the first one.
    val currentSignature = rememberUpdatedState(signature)

    DisposableEffect(Unit) {
        onDispose {
            webView?.destroy()
            webView = null
        }
    }

    val title = when (mode) {
        RichComposeMode.New -> L10n.compose_title_new(ctx)
        RichComposeMode.Reply -> L10n.action_reply(ctx)
        RichComposeMode.ReplyAll -> L10n.action_reply_all(ctx)
        RichComposeMode.Forward -> L10n.action_forward(ctx)
    }

    // Closing the composer, the ✕ AND the system back, which is the whole point: one of them is a
    // deliberate tap and the other is an edge swipe you can make by accident, and they must not
    // differ about whether a half-written message survives. A clean composer closes silently; there
    // is nothing to lose, and stopping to say so would be noise.
    //
    // The body check is asynchronous (a hop into the WebView), so it runs only here, at the one
    // moment the answer is needed, rather than on every keystroke.
    val requestDismiss = dismiss@{
        if (composerHeadersEdited(
                to, seededTo, cc, seededCc, bcc, seededBcc, subject, initialSubject,
                attachments.size,
            )
        ) {
            confirmingDiscard = true
            return@dismiss
        }
        val view = webView
        if (view == null || seedDocument == null) {
            onDismiss()
            return@dismiss
        }
        view.evaluateJavascript("composerDocument()") { encoded ->
            if (decodeJsString(encoded) == seedDocument) onDismiss() else confirmingDiscard = true
        }
    }

    val send = send@{
        prepareError = false
        val webViewOrNull = webView
        if (webViewOrNull == null) {
            prepareError = true
            return@send
        }
        webViewOrNull.evaluateJavascript("composerDocument()") { encoded ->
            val documentJson = decodeJsString(encoded)
            if (documentJson != null &&
                onSubmitRich(
                    from?.id,
                    Recipients(to, cc, bcc),
                    subject,
                    documentJson,
                    attachments.map { it.composerFile },
                )
            ) {
                onDismiss()
            } else {
                prepareError = true
            }
        }
    }

    // usePlatformDefaultWidth = false is what lets the dialog fill the viewport rather than sit in
    // the platform's inset alert-dialog box. The system back button reaches onDismissRequest, so
    // back leaves the composer exactly like the Close button, including the discard guard both
    // now route through.
    //
    // decorFitsSystemWindows = false is required alongside it. Left at its default the dialog
    // window is *floating*, so the system adjusts it for the keyboard itself (a pan that scrolls
    // the app bar off-screen) while `imePadding()` below independently shrinks the content, the
    // keyboard is subtracted twice and the editor collapses to a sliver. Cleared, the window is
    // full-screen with SOFT_INPUT_ADJUST_NOTHING, so `imePadding()` is the single adjustment.
    // It also means the window now spans the status/navigation bars; the Scaffold's TopAppBar and
    // content insets pad for them.
    Dialog(
        onDismissRequest = requestDismiss,
        properties = DialogProperties(
            usePlatformDefaultWidth = false,
            decorFitsSystemWindows = false,
        ),
    ) {
        // A dialog gets its own window, which does not inherit the activity's system-bar icon
        // appearance. Now that the composer draws behind those bars, say so explicitly, or the
        // status-bar icons stay light and vanish against the light composer.
        SystemBarsMatchTheme()
        Surface(modifier = Modifier.fillMaxSize()) {
            Scaffold(
                modifier = Modifier.imePadding(),
                topBar = {
                    TopAppBar(
                        title = { Text(title) },
                        navigationIcon = {
                            IconButton(onClick = requestDismiss) {
                                Icon(
                                    painter = painterResource(R.drawable.ic_close),
                                    contentDescription = L10n.action_cancel(ctx),
                                )
                            }
                        },
                        actions = {
                            // Attach + Signature + Send as app-bar icons, like Gmail/Thunderbird on
                            // Android. The app bar IS this platform's action bar, which is where
                            // docs/signatures.md puts the signature control: it is an action you
                            // take on the message, not a field you address it with. (macOS and iOS
                            // draw the same three as a bar above the editor, having no app bar.)
                            IconButton(onClick = { pickAttachments.launch(arrayOf("*/*")) }) {
                                Icon(
                                    painter = painterResource(R.drawable.ic_attachment),
                                    contentDescription = L10n.action_attach(ctx),
                                )
                            }
                            // Only once the user has written a signature: with an empty library the
                            // menu would offer nothing but "None", a control that cannot do anything.
                            if (signatures != null && signatures.library.isNotEmpty()) {
                                ComposerSignatureAction(
                                    library = signatures.library,
                                    selected = signature?.id,
                                    expanded = signatureMenuOpen,
                                    onExpand = { signatureMenuOpen = it },
                                    onSelect = { id ->
                                        signatureChoice = id?.let(SignatureChoice::Named)
                                            ?: SignatureChoice.NoSignature
                                        webView?.setComposerSignature(
                                            signatures.resolve(signatureChoice, from?.id, mode),
                                        )
                                    },
                                )
                            }
                            IconButton(
                                enabled = to.isNotBlank() && from != null,
                                onClick = send,
                            ) {
                                Icon(
                                    painter = painterResource(R.drawable.ic_send),
                                    contentDescription = L10n.action_send(ctx),
                                )
                            }
                        },
                    )
                },
            ) { padding ->
                // Keeps the editor's top inset in step with *later* header-height changes (revealing
                // Cc/Bcc grows the overlay). The initial value is sent from onPageFinished instead:
                // this effect runs on the layout pass that measures the header, which is frames
                // before the editor bundle has parsed, so on first open the hook it calls does not
                // exist yet and the call is silently dropped.
                LaunchedEffect(headerHeightPx) {
                    if (headerHeightPx > 0) {
                        webView?.evaluateJavascript(
                            "window.setComposerTopInset(${headerHeightPx / density.density})",
                            null,
                        )
                    }
                }
                // Auto-swap: the signature follows the sender, because a work signature under a
                // personal address is the mistake this setting exists to prevent. A no-op once the
                // user has picked one explicitly, they chose it for this message, and swapping it
                // under them would undo that. On the first composition the editor has not parsed
                // yet, so this call is dropped and the page-finished seed is what lands.
                LaunchedEffect(from?.id) {
                    if (signatureChoice == null) {
                        webView?.setComposerSignature(signature)
                    }
                }
                Box(
                    modifier = Modifier
                        .fillMaxSize()
                        .padding(padding)
                        // A dropped file is resolved natively: web code sees a `File` with no path,
                        // so only the host can stage it for Rust and list it for removal.
                        .composerDropTarget { dropped ->
                            attachments = attachments + dropped.attach
                            droppedPictures = dropped.pictures
                        },
                ) {
                    // The WebView fills the surface and owns the scroll (its toolbar pins above the
                    // keyboard via position:fixed in editor.html). It reports its scroll offset so
                    // the header overlay above can track it.
                    AndroidView(
                        modifier = Modifier
                            .fillMaxSize()
                            // The WebView must draw into a layer of its own. Without one its first
                            // paint washes over the header overlay above it, which stays laid out
                            // and tappable while invisible, so a tap on what looks like the body
                            // lands in a hidden address field, and the typing never reaches the
                            // editor.
                            .graphicsLayer { clip = true },
                        factory = { context ->
                            WebView(context).apply {
                                configureComposerWebView(
                                    quote = quote,
                                    body = initialBody,
                                    labelsJson = labelsJson,
                                    // The caret opens where the work starts, the body here, the
                                    // To field otherwise (`composerOpensInBody`). Raising the
                                    // keyboard is part of it, rather than making the user tap first.
                                    focusBody = composerOpensInBody(mode, initialTo),
                                    // Read at page-finished time, not captured now: the header has
                                    // been measured by then, so the editor gets its real inset.
                                    topInsetDp = { headerHeightPx / density.density },
                                    signature = { signatureSeedJson(currentSignature.value) },
                                    onScroll = { y -> scrollY = y },
                                    onSeeded = { seeded -> seedDocument = seeded },
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
                    // The address-field header, overlaid on the WebView and offset by the scroll so
                    // it scrolls up and off as the message grows. Opaque, so the empty editor area
                    // it covers doesn't show through between the fields.
                    Column(
                        modifier = Modifier
                            .fillMaxWidth()
                            .onSizeChanged { headerHeightPx = it.height }
                            .graphicsLayer { translationY = -scrollY.toFloat() }
                            .background(MaterialTheme.colorScheme.surface)
                            .padding(horizontal = 16.dp)
                            .padding(top = 8.dp),
                    ) {
                        ComposerHeaderFields(
                            accounts = accounts,
                            from = from,
                            onFrom = { from = it },
                            to = to,
                            onTo = { to = it },
                            cc = cc,
                            onCc = { cc = it },
                            bcc = bcc,
                            onBcc = { bcc = it },
                            subject = subject,
                            onSubject = { subject = it },

                            showCcBcc = showCcBcc,
                            onToggleCcBcc = { showCcBcc = !showCcBcc },
                            style = style.takeIf {
                                ComposerQuote.showsStylePicker(quote != null, quoteStylePerMessage)
                            },
                            onStyle = { picked ->
                                style = picked
                                webView?.evaluateJavascript(
                                    "window.setComposerQuoteStyle(${JSONObject.quote(ComposerQuote.token(picked))})",
                                    null,
                                )
                            },
                            suggestionsFor = suggestionsFor,
                            focusesTo = !composerOpensInBody(mode, initialTo),
                        )
                        ComposerAttachmentList(
                            attachments = attachments,
                            onRemove = { attachment ->
                                attachments = attachments.filterNot { it.id == attachment.id }
                                File(attachment.path).delete()
                            },
                        )
                        if (prepareError) {
                            Text(
                                text = L10n.compose_prepare_error(ctx),
                                color = MaterialTheme.colorScheme.error,
                                style = MaterialTheme.typography.bodySmall,
                                modifier = Modifier.padding(bottom = 8.dp),
                            )
                        }
                    }
                    // Inside the composer's own Dialog, so its window is created after this one and
                    // stacks above it, and so back reaches the confirmation (keep editing) rather
                    // than the composer underneath.
                    // The dropped-picture question. Drawn inside the composer's own Dialog for the
                    // same reason the discard prompt is: its window then stacks above, and back
                    // reaches the question rather than the composer under it.
                    ComposerDroppedPictureQuestion(
                        pictures = droppedPictures,
                        webView = webView,
                        onAttach = { attachments = attachments + it },
                        onUnreadable = { prepareError = true },
                        onAnswered = { droppedPictures = emptyList() },
                    )
                    if (confirmingDiscard) {
                        DiscardDraftDialog(
                            onDiscard = {
                                confirmingDiscard = false
                                onDismiss()
                            },
                            onKeepEditing = { confirmingDiscard = false },
                        )
                    }
                }
            }
        }
    }
}

// Point the enclosing dialog window's status/navigation-bar icons at the current theme, so they
// stay legible against the composer drawing behind them. A no-op outside a dialog.
@Composable
internal fun SystemBarsMatchTheme() {
    val view = LocalView.current
    val lightBars = !LocalAppDark.current
    SideEffect {
        val window = (view.parent as? DialogWindowProvider)?.window ?: return@SideEffect
        WindowCompat.getInsetsController(window, view).apply {
            isAppearanceLightStatusBars = lightBars
            isAppearanceLightNavigationBars = lightBars
        }
    }
}
