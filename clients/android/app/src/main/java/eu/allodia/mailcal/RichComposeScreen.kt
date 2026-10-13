// The rich compose / reply / forward surface. A FULL-SCREEN dialog (not the old AlertDialog, which
// only used part of the viewport): a top app bar with Close + Send, the address fields, and the
// editor filling everything that's left. The WebView loads only the bundled editor asset; Rust owns
// validation/rendering through submitComposition.
//
// It stays a `Dialog` rather than a screen swapped into MainActivity's `when`, so it sits over
// whichever screen raised it (MainActivityComposer.kt decides whether it is open) and the system back
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
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import java.io.File
import org.json.JSONObject
import uniffi.mailcal_bindings.AccountRow
import uniffi.mailcal_bindings.DraftStatus
import uniffi.mailcal_bindings.RecipientMatch
import uniffi.mailcal_bindings.Recipients
import uniffi.mailcal_bindings.senderLabel

@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun RichComposeMessageDialog(
    // What the core opened this composer with, and what the host derived from it
    // (`composeContext`). Every route into a composer arrives as one of these.
    request: ComposeContext,
    onDismiss: () -> Unit,
    // Everything the send names, in one value: the account picked in the From dropdown, the
    // recipients, the subject, the rendered document, the files, and the composition it was
    // written in (`ComposerSubmission`).
    onSubmitRich: (ComposerSubmission) -> Boolean,
    // Every configured account, for the From dropdown, which opens on `request.initialFrom`.
    accounts: List<AccountRow>,
    // Ranked recipient suggestions for a partially-typed address, answered by the core from synced
    // contacts and from people the user has written to. Null disables autosuggest (screenshot runs
    // and tests, which must not depend on a populated people index).
    suggestionsFor: ((String) -> List<RecipientMatch>)? = null,
    // The signature library + the two lookups the core answers, or null to leave signatures out of
    // this composer entirely (a screenshot run, a test).
    signatures: ComposerSignatures? = null,
    // The core verbs this composer keeps its message on the server with, or null to turn draft
    // saving off (a screenshot run, a test).
    drafts: ComposerDrafts? = null,
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
    var from by remember(request.initialFrom, accounts) {
        mutableStateOf(accounts.firstOrNull { it.id == request.initialFrom } ?: accounts.firstOrNull())
    }
    // Every address the caller pre-filled is finished, nothing here is being typed, so the fields
    // open with all of them committed and each renders as its own pill (see `seededRecipientField`).
    // The normalised values are what the dirty check below compares against, too: compare against
    // the RAW ones and every reply would open already counting as an edited draft, and ask before
    // closing something nobody typed into.
    val opening = request.opening
    val seededTo = seededRecipientField(opening.to)
    val seededCc = seededRecipientField(opening.cc)
    val seededBcc = seededRecipientField(opening.bcc)
    var to by remember { mutableStateOf(seededTo) }
    var cc by remember { mutableStateOf(seededCc) }
    var bcc by remember { mutableStateOf(seededBcc) }
    var subject by remember { mutableStateOf(opening.subject) }
    // Cc/Bcc are tucked away by default (matching Gmail/Thunderbird) and revealed by the chevron on
    // the To row. Opened from the start when either arrives pre-filled, so those addresses are
    // never hidden behind a tap the user doesn't know to make.
    var showCcBcc by remember { mutableStateOf(revealsCcBcc(opening.cc, opening.bcc)) }
    // The one error line under the composer, which more than one failure writes to: a send that
    // couldn't be prepared, a dropped picture that couldn't be shown, a forward whose files
    // couldn't be read. It carries the message rather than a flag, so each says which it is. A
    // forward whose originals could not be staged opens saying so: silence would be a message the
    // user sends believing the original had nothing attached.
    var composerError by remember {
        mutableStateOf(L10n.compose_forward_attachments_failed(ctx).takeIf { opening.attachmentsFailed })
    }
    var webView by remember { mutableStateOf<WebView?>(null) }
    val linkHost = rememberComposerLinkHost()
    // The single-scroll model: the WebView owns the one scroll (so its native caret-following and
    // drag-to-scroll just work), and the address-field header is a native overlay drawn on top of
    // it whose vertical offset tracks the WebView's scroll, so it scrolls away as the message
    // grows, exactly like Thunderbird. `headerHeightPx` is fed back to the editor as its top inset
    // so the text starts just below the overlay and the two move in lockstep.
    var scrollY by remember { mutableIntStateOf(0) }
    var headerHeightPx by remember { mutableIntStateOf(0) }
    // Seeded once, from what the core staged (a forward's originals, a stored draft's own) and a
    // share's files. Deliberately NOT keyed on the seed: a key would let a share arriving over an
    // OPEN composer replace the files the user had picked. A share reaching a busy composer waits
    // (MainActivityComposer.kt), it does not overwrite.
    var attachments by remember { mutableStateOf(request.attachments.map(::seededComposerFile)) }
    // Pictures dropped on the composer, waiting on the one question they raise. Held rather than
    // acted on, because the answer decides whether they become body content or attachments.
    var droppedPictures by remember { mutableStateOf<List<PickedComposerFile>>(emptyList()) }
    val pickAttachments = rememberAttachmentPicker(
        onStaged = { attachments = attachments + it },
        onFailed = { composerError = L10n.compose_prepare_error(ctx) },
    )
    // The per-message style override of the persisted default, re-styles the quoted original in
    // place without disturbing the user's typed message.
    var style by remember { mutableStateOf(request.quoteStyle) }
    // The user's explicit signature choice for this message, or null to follow the From account.
    var signatureChoice by remember { mutableStateOf<SignatureChoice?>(null) }
    var signatureMenuOpen by remember { mutableStateOf(false) }
    // The editor document as it stood once the quote and signature were seeded, the baseline the
    // written-work check compares against, so a reply that merely carries its quoted original does
    // not open already written in. Null until the editor answers, which reads as "nothing to lose".
    var seedDocument by remember { mutableStateOf<String?>(null) }
    var confirmingDiscard by remember { mutableStateOf(false) }
    // The composition the core joined to this composer when it opened: every save, the discard
    // and the send name it. One minted here would store a second draft beside a resumed one, and
    // send a reply that had lost its conversation (`docs/drafts.md`).
    val compositionId = opening.composition
    // How many changes the composer has seen, header and body alike. Every one restarts the idle
    // interval; the value itself is never read for anything else.
    var draftChanges by remember { mutableIntStateOf(0) }
    // The header fields, as one value, so one effect covers all of them rather than one per field.
    // The pre-fill arrives as each field's *initial* value, so it raises no change and no composer
    // opens counting as edited.
    val draftHeaders = listOf(to, cc, bcc, subject, attachments.size.toString())
    LaunchedEffect(draftHeaders) { draftChanges += 1 }
    // The most recent save's state, re-pulled whenever the core says some composition's moved.
    var draftStatus by remember { mutableStateOf(DraftStatus.IDLE) }
    // Whether this composer's message was submitted. From then on the send owns the composition
    // and finishes with it when the message settles, so forgetting it here as well would race the
    // cleanup that takes the stored draft away (docs/drafts.md).
    var draftSubmitted by remember { mutableStateOf(false) }
    // Whether the composer was left through a call that already finished with the composition: a
    // save-and-close or a discard. Disposal then has nothing left to forget.
    var draftLeft by remember { mutableStateOf(false) }
    // A stored message carries its own signature, so a composer the core says seeds none never
    // touches the signature region: not on open, not on a From change, and it offers no picker.
    val signing = ComposeOpeningSeed.signing(signatures, opening)
    val mode = request.mode
    // Resolved fresh on every read rather than held in state: the account's assignment can change
    // under an open composer (Settings is reachable from the notification shade), and this is a
    // cheap in-memory core lookup.
    val signature = signing?.resolve(signatureChoice, from?.id, mode)
    // The factory below runs once, but the editor finishes parsing later, and `from` can still
    // settle in between (the account list arrives after the composer opened). Hold the latest
    // resolution so the page-finished seed is the current one, not the first one.
    val currentSignature = rememberUpdatedState(signature)

    DisposableEffect(Unit) {
        onDispose {
            webView?.destroy()
            webView = null
            // A composer that closed with nothing to keep. Without it the core holds a record per
            // composer for the life of the process, and the stored draft would be superseded by
            // whatever the next composer to take this id wrote. Never after a submit: the send
            // finishes with the composition itself (docs/drafts.md).
            if (!draftSubmitted && !draftLeft) {
                drafts?.close(compositionId)
            }
        }
    }

    val title = when (mode) {
        RichComposeMode.New -> L10n.compose_title_new(ctx)
        RichComposeMode.Reply -> L10n.action_reply(ctx)
        RichComposeMode.ReplyAll -> L10n.action_reply_all(ctx)
        RichComposeMode.Forward -> L10n.action_forward(ctx)
    }

    // What the composer holds, for a send or a save, always on the composition it opened on, so an
    // accepted send takes the stored copy away (`docs/drafts.md`).
    val contentOf = { documentJson: String ->
        ComposerSubmission(
            from = from?.id,
            recipients = Recipients(to, cc, bcc),
            subject = subject,
            documentJson = documentJson,
            files = attachments.map { it.composerFile },
            composition = compositionId,
        )
    }

    val send = send@{
        composerError = null
        val webViewOrNull = webView
        if (webViewOrNull == null) {
            composerError = L10n.compose_prepare_error(ctx)
            return@send
        }
        webViewOrNull.evaluateJavascript("composerDocument()") { encoded ->
            val documentJson = decodeJsString(encoded)
            if (documentJson != null && onSubmitRich(contentOf(documentJson))) {
                draftSubmitted = true
                onDismiss()
            } else {
                composerError = L10n.compose_prepare_error(ctx)
            }
        }
    }

    val exits = ComposerExits(
        drafts = drafts,
        composition = compositionId,
        headersEdited = {
            composerHeadersEdited(
                to, seededTo, cc, seededCc, bcc, seededBcc, subject, opening.subject,
                attachments.size, request.baselineAttachments,
            )
        },
        readDocument = { answer ->
            webView?.evaluateJavascript("composerDocument()") { answer(decodeJsString(it)) }
                ?: answer(null)
        },
        seedDocument = { seedDocument },
        contentOf = contentOf,
        ask = { confirmingDiscard = true },
        left = { draftLeft = true },
        dismiss = onDismiss,
    )

    // usePlatformDefaultWidth = false is what lets the dialog fill the viewport rather than sit in
    // the platform's inset alert-dialog box. The system back button reaches onDismissRequest, so
    // back leaves the composer exactly like the Close button.
    //
    // decorFitsSystemWindows = false is required alongside it. Left at its default the dialog
    // window is *floating*, so the system adjusts it for the keyboard itself (a pan that scrolls
    // the app bar off-screen) while `imePadding()` below independently shrinks the content, the
    // keyboard is subtracted twice and the editor collapses to a sliver. Cleared, the window is
    // full-screen with SOFT_INPUT_ADJUST_NOTHING, so `imePadding()` is the single adjustment.
    // It also means the window now spans the status/navigation bars; the Scaffold's TopAppBar and
    // content insets pad for them.
    Dialog(
        onDismissRequest = exits::leave,
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
                    ComposerTopBar(
                        title = title,
                        onClose = exits::leave,
                        onAttach = { pickAttachments.launch(arrayOf("*/*")) },
                        signaturePicker = signing
                            ?.takeIf { it.library.isNotEmpty() }
                            ?.let { library ->
                                {
                                    ComposerSignatureAction(
                                        library = library.library,
                                        selected = signature?.id,
                                        expanded = signatureMenuOpen,
                                        onExpand = { signatureMenuOpen = it },
                                        onSelect = { id ->
                                            signatureChoice = id?.let(SignatureChoice::Named)
                                                ?: SignatureChoice.NoSignature
                                            webView?.setComposerSignature(
                                                library.resolve(signatureChoice, from?.id, mode),
                                            )
                                        },
                                    )
                                }
                            },
                        onSaveDraft = (exits::save).takeIf { drafts != null },
                        onDiscard = (exits::discard).takeIf { drafts != null },
                        sendEnabled = to.isNotBlank() && from != null,
                        onSend = send,
                    )
                },
            ) { padding ->
                // Keeps the editor's top inset in step with *later* header-height changes (revealing
                // Cc/Bcc grows the overlay). The initial value is sent from onPageFinished instead:
                // this effect runs on the layout pass that measures the header, which is frames
                // before the editor bundle has parsed, so on first open the hook it calls does not
                // exist yet and the call is silently dropped.
                ComposerDraftEffects(
                    drafts = drafts,
                    composition = compositionId,
                    webView = webView,
                    changes = draftChanges,
                    onChanged = { draftChanges += 1 },
                    onStatus = { draftStatus = it },
                    onIdle = exits::save,
                )
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
                    if (signing != null && signatureChoice == null) {
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
                    ComposerEditorView(
                        html = html,
                        linkHost = linkHost,
                        quote = request.quote,
                        body = request.initialBody,
                        storedHtml = request.storedHtml,
                        quoteAttribution = request.quoteAttribution,
                        labelsJson = labelsJson,
                        // The caret opens where the work starts, the body here, the To field
                        // otherwise (`composerOpensInBody`). Raising the keyboard is part of it,
                        // rather than making the user tap first.
                        focusBody = request.opensInBody,
                        // Read at page-finished time, not captured now: the header has been
                        // measured by then, so the editor gets its real inset.
                        topInsetDp = { headerHeightPx / density.density },
                        signature = { signatureSeedJson(currentSignature.value) },
                        onScroll = { y -> scrollY = y },
                        onSeeded = { seeded ->
                            seedDocument = seeded
                            // A reopened quote keeps the style it was saved in, so the picker
                            // opens on that rather than on the app default.
                            if (request.storedHtml.isNotEmpty()) {
                                ComposeOpeningSeed.quoteStyleOf(seeded)?.let { style = it }
                            }
                        },
                        onReady = { webView = it },
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
                            // `Name <address>`, or the address alone: the core owns the empty
                            // case, so no client decides it (docs/sending.md rule 7).
                            fromLabel = { senderLabel(it.name, it.email) },
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
                            style = style.takeIf { request.showsStylePicker },
                            onStyle = { picked ->
                                style = picked
                                webView?.evaluateJavascript(
                                    "window.setComposerQuoteStyle(${JSONObject.quote(ComposerQuote.token(picked))})",
                                    null,
                                )
                            },
                            suggestionsFor = suggestionsFor,
                            focusesTo = !request.opensInBody,
                        )
                        ComposerAttachmentList(
                            attachments = attachments,
                            onRemove = { attachment ->
                                attachments = attachments.filterNot { it.id == attachment.id }
                                File(attachment.path).delete()
                            },
                        )
                        ComposerNotices(error = composerError, draftStatus = draftStatus)
                    }
                    // The dropped-picture question. Drawn inside the composer's own Dialog so its
                    // window is created after this one and stacks above it, and so back reaches the
                    // question rather than the composer under it.
                    ComposerDroppedPictureQuestion(
                        pictures = droppedPictures,
                        webView = webView,
                        onAttach = { attachments = attachments + it },
                        onUnreadable = { composerError = L10n.compose_image_failed(ctx) },
                        onAnswered = { droppedPictures = emptyList() },
                    )
                    ComposerLinkDialogHost(linkHost, webView)
                    // Inside the composer's own Dialog for the same reason, so back reaches the
                    // confirmation (keep editing) rather than the composer underneath.
                    if (confirmingDiscard) {
                        DiscardDraftDialog(
                            onDiscard = {
                                confirmingDiscard = false
                                // The one path that takes the stored copy off the server; leaving
                                // the composer any other way keeps the draft in Drafts.
                                exits.confirmDiscard()
                            },
                            onKeepEditing = { confirmingDiscard = false },
                        )
                    }
                }
            }
        }
    }
}
