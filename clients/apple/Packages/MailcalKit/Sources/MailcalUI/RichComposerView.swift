// The rich composer for the shared Apple client. The editor document is trusted bundled code
// loaded in a dedicated WKWebView; the body output is still produced by shared Rust
// (`submitComposition` validates and renders before queuing the send).

import Foundation
import MailcalBindings
import SwiftUI
import WebKit

private struct RichComposerWebView: PlatformViewRepresentable {
    let editor: RichComposerEditor

    #if os(macOS)
    func makeNSView(context: Context) -> WKWebView { mount(context) }
    func updateNSView(_ nsView: WKWebView, context: Context) { followScheme(context) }
    #else
    func makeUIView(context: Context) -> WKWebView { mount(context) }
    func updateUIView(_ uiView: WKWebView, context: Context) { followScheme(context) }
    #endif

    private func mount(_ context: Context) -> WKWebView {
        let view = editor.mount()
        followScheme(context)
        return view
    }

    private func followScheme(_ context: Context) {
        editor.host.setDark(context.environment.colorScheme == .dark)
    }
}

/// The column's padding, around everything in it.
private let columnPadding: CGFloat = 16

/// The gap between the column's sections.
private let columnSpacing: CGFloat = 12

/// What the rich composer is for: its heading and its signature slot. Every mode exposes editable
/// To/Cc/Bcc/Subject fields, opened with what the core answered (`ComposeOpening`), so the user
/// can adjust anything before sending. Every mode shares the one hardened editor host.
enum RichComposeMode {
    case new
    case reply
    case replyAll
    case forward
}

/// Rich composer for a new message, reply, reply-all, or forward: the shared local editor
/// bundle plus the editable From/To/Cc/Bcc/Subject header fields. `send` receives the entered
/// [`Recipients`], the Subject as edited, the rendered document JSON, and the id of the account
/// picked in the From dropdown; the parent sends it through `submitComposition`. Every mode passes the `accounts` to
/// choose a sender from plus the `initialFrom` it opens on.
struct RichComposeView: View {
    let title: String
    var mode: RichComposeMode = .new
    let accounts: [AccountRow]
    let send: (ComposerSubmission) -> Bool
    let cancel: () -> Void
    /// Ranked address suggestions for a partially-typed recipient, answered by the core from synced
    /// contacts **and** from people the user has written to before, so it works on an account with
    /// no address book at all. `nil` disables autosuggest (a preview or a screenshot run).
    var suggestionsFor: ((String) async -> [RecipientMatch])?
    /// The signature library + lookups, or `nil` to disable signatures for this composer.
    var signatures: ComposerSignatures?
    /// The core verbs this composer keeps its draft on the server with, or `nil` to turn draft
    /// saving off (a preview, a screenshot run).
    var drafts: ComposerDrafts?
    /// Moves this composer into a window of its own, answering whether it moved; `nil` hides the
    /// pop-out button. Not `private`: RichComposerView.PopOut.swift calls it.
    let popOut: ((ComposerState) async -> Bool)?
    /// Whether the composer this one was moved from had been written in: then this one has too,
    /// whatever it opened holding (`MovedComposer.written`).
    let openedWritten: Bool
    /// Whether this compose shows the per-message quote-style picker: it carries a quoted original
    /// (reply/forward) *and* the user opted into per-message styling in Settings.
    private let showsStylePicker: Bool
    // Not `private`: RichComposerView.Signature.swift's signature-resolution properties read and
    // set it too.
    @State var editor: RichComposerEditor
    @State private var from: String?
    /// The user's explicit signature choice for this message, or `nil` to follow the account.
    /// Not `private`: RichComposerView.Signature.swift reads and sets it too.
    @State var signatureChoice: SignatureChoice?
    @State var to: String
    @State var cc: String
    @State var bcc: String
    /// Whether the Cc/Bcc rows are revealed. Collapsed unless the caller pre-filled one.
    @State private var showsCcBcc: Bool
    @State var subject: String
    /// This composer's composition, for as long as it is open: the one the core joined to the
    /// message when the composer opened, and what every draft call and the send name. Per composer
    /// because a desktop can have several open at once (`docs/reading-window.md`).
    @State var composition: String
    /// The most recent save's state, re-pulled whenever the core says some composition's moved.
    @State var draftStatus: DraftStatus = .idle
    /// How many changes the composer has seen. Each one restarts the idle interval.
    @State var draftChanges = 0
    /// Whether this composer's composition was handed over, by a submit or a discard: each
    /// finishes with it, so leaving must not close it as well (`docs/drafts.md`).
    @State var draftReleased = false
    /// Whether "Discard draft?" is up.
    @State var confirmingDiscard = false
    /// The header fields as the composer opened, which "was anything written?" compares against.
    let openingHeaders: DraftHeaders
    /// The one error line under the composer, which more than one failure writes to: a send that
    /// could not be prepared, a dropped picture that could not be shown, and a forward whose
    /// files could not be read. It carries the message rather than a flag, so each says which.
    /// Not `private`: RichComposerView.Drop.swift sets it when a dropped picture cannot be shown.
    @State var composerError: String?
    /// Pictures dropped on the composer, waiting on the one question they raise. Held rather than
    /// acted on, because the answer decides whether they become body content or attachments.
    /// Not `private`: RichComposerView.Drop.swift owns the question.
    @State var droppedPictures: [URL] = []
    @State private var quoteStyle: QuoteStyleKind
    /// The composition's files as the core last answered with them. Not `private`:
    /// RichComposerView.Attachments.swift draws and changes them.
    @State var files: ComposerFiles
    /// The core verbs the files go through.
    let fileVerbs: ComposerFileVerbs
    /// Whether the To field takes the caret when the composer appears, the other half of
    /// `opensInBody`, so exactly one of the two is focused.
    private let focusesTo: Bool
    /// The height of everything above the editor in the column, so the editor can take what is
    /// left of the visible part.
    @State private var aboveEditorHeight: CGFloat = 0
    /// The column's scroll offset and the height of its visible part.
    @State private var visible = ScrollWindow(offset: 0, height: 0)
    /// Where the column is scrolled to, which following the caret moves.
    @State private var scrollPosition = ScrollPosition(edge: .top)

    init(
        title: String,
        mode: RichComposeMode = .new,
        accounts: [AccountRow] = [],
        initialFrom: String? = nil,
        initialTo: String = "",
        initialCc: String = "",
        initialBcc: String = "",
        initialSubject: String = "",
        initialBody: String = "",
        /// The HTML of a message this composer reopens (`ComposeOpening.bodyHtml`), which the
        /// editor reads back into its document; `initialBody` is then that message's text.
        initialHTML: String = "",
        /// An error the composer opens showing, for a failure that happened before it did: the
        /// files a forward was to carry could not be read.
        initialError: String? = nil,
        /// The composition the core opened this composer on (`ComposeOpening.composition`).
        composition: String,
        quote: String? = nil,
        /// The `completeComposerQuoteAttribution` argument for a reopened quote, or `nil`.
        quoteAttribution: String? = nil,
        quoteStyle: QuoteStyleKind = .indented,
        quoteStylePerMessage: Bool = false,
        suggestionsFor: ((String) async -> [RecipientMatch])? = nil,
        signatures: ComposerSignatures? = nil,
        drafts: ComposerDrafts? = nil,
        /// The core verbs this composer's files go through (`docs/composer-layout.md`, rule 3).
        files: ComposerFileVerbs,
        /// What a composer moved from another view brings besides its fields and files
        /// (`ComposeOpening.moved`): it opens on that document and nothing else is seeded.
        moved: MovedComposer? = nil,
        popOut: ((ComposerState) async -> Bool)? = nil,
        send: @escaping (ComposerSubmission) -> Bool,
        cancel: @escaping () -> Void
    ) {
        self.title = title
        self.mode = mode
        self.accounts = accounts
        self.send = send
        self.cancel = cancel
        self.suggestionsFor = suggestionsFor
        self.signatures = signatures
        self.drafts = drafts
        self.popOut = popOut
        self.openedWritten = moved?.written ?? false
        // A moved composer's quote is in its document, in the style it was left in.
        let movedQuoteStyle = moved.flatMap { ComposeOpeningSeed.quoteStyle(ofDocument: $0.documentJson) }
        self.showsStylePicker = ComposerQuote.showsStylePicker(
            hasQuote: quote != nil || quoteAttribution != nil || movedQuoteStyle != nil,
            perMessage: quoteStylePerMessage
        )
        self.focusesTo = !Self.opensInBody(mode: mode, to: initialTo)
        let editor = RichComposerEditor()
        editor.pendingQuote = quote
        editor.pendingPlainBody = initialBody.isEmpty ? nil : initialBody
        editor.pendingStoredHTML = initialHTML.isEmpty ? nil : initialHTML
        editor.pendingQuoteAttribution = quoteAttribution
        editor.pendingDocument = moved?.documentJson
        // The opening signature is resolved here, not in `onAppear`: it has to be injected before
        // the editor snapshots its "nothing written yet" seed, or the composer opens dirty. The
        // account is resolved the same way `resolvedFrom` does below, the From dropdown and the
        // signature must agree about who is sending. A moved composer's document already carries
        // the signature it had, so nothing is seeded over it.
        let opening = FromAccountField.sender(initialFrom, in: accounts)
        editor.pendingSignature = moved != nil ? nil : opening
            .flatMap { signatures?.forAccount($0, signatureSlot(for: mode)) }
            .flatMap(Self.signatureSeed)
        _signatureChoice = State(initialValue: moved.flatMap { Self.signatureChoice($0.signature) })
        // The caret opens where the work starts. A reply/forward is already addressed, so writing
        // is the only thing left to do and the body takes it; a new message's To is empty and is
        // where the user has to begin. An assistant's draft is the exception among new messages:
        // it supplied the recipients, so the body is the place there too.
        editor.focusBodyOnLoad = Self.opensInBody(mode: mode, to: initialTo)
        _editor = State(initialValue: editor)
        _from = State(initialValue: initialFrom)
        // Every address the caller pre-filled is finished, nothing here is being typed, so the
        // fields open with all of them committed, and each renders as its own pill (see
        // `seededRecipientField`). Normalising at the *initial* value rather than in an `onAppear`
        // also keeps the draft clean: the tracking below watches for a CHANGE, and a rewrite one
        // frame after opening would read as the user having edited the recipients.
        _to = State(initialValue: seededRecipientField(initialTo))
        _cc = State(initialValue: seededRecipientField(initialCc))
        _bcc = State(initialValue: seededRecipientField(initialBcc))
        // Collapsed by default, but never over a recipient the caller put there. A reply-all fills
        // Cc, and an assistant's draft may fill either, and a recipient the sender cannot see is one
        // they cannot remove (docs/composer-security.md, Gate 12).
        _showsCcBcc = State(initialValue: revealsCcBcc(cc: initialCc, bcc: initialBcc))
        _subject = State(initialValue: initialSubject)
        openingHeaders = DraftHeaders(
            to: seededRecipientField(initialTo),
            cc: seededRecipientField(initialCc),
            bcc: seededRecipientField(initialBcc),
            subject: initialSubject,
            filesChanged: false
        )
        _composition = State(initialValue: composition)
        // What its save last came to: a moved composer's is the one it left's, and any other's is
        // `.idle` until it saves.
        _draftStatus = State(initialValue: drafts?.status(composition) ?? .idle)
        _quoteStyle = State(initialValue: movedQuoteStyle ?? quoteStyle)
        fileVerbs = files
        _files = State(initialValue: files.read(composition))
        _composerError = State(initialValue: initialError)
    }

    /// Whether this composer opens with the caret in the message body rather than in To.
    ///
    /// The two are exclusive, and every client decides it the same way (docs/contacts.md §4).
    ///
    /// `nonisolated` because the rule is a pure function of its arguments; it inherits the main
    /// actor from `View` otherwise, which its unit tests would then have to pretend to be.
    nonisolated static func opensInBody(mode: RichComposeMode, to: String) -> Bool {
        mode != .new || !to.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
    }

    var sendDisabled: Bool {
        to.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
    }

    /// The account the From field shows *and* the one the send goes out as
    /// (`FromAccountField.sender(_:in:)`).
    ///
    /// Not `private`: RichComposerView.Signature.swift's `accountSignature` reads it too.
    var resolvedFrom: String? {
        FromAccountField.sender(from, in: accounts)
    }

    var body: some View {
        #if os(macOS)
        // macOS: the composer fills the detail column, in place of the reading pane.
        // Deliberately NOT the fixed 620x560 it carried as a window-sheet, the column is
        // user-resizable (the list|reading splitter), so a fixed frame would sit stranded in it and
        // clip its own editor at a narrow width. Everything stretches; the editor takes the slack.
        // Send and Cancel live in the action bar above the editor now (see `actionBar`), not in a
        // row under it, so the message's actions sit together instead of straddling the body.
        composerColumn {
            // The pop-out sits at the composer's top right, where Outlook puts it: it is about the
            // composer as a whole, not an action on the message the bar below holds.
            HStack {
                Text(title).font(.headline)
                Spacer()
                popOutButton
            }
        }
        // On the composer's outermost view, so an open recipient list clears the editor's web
        // view. Nearer the field it is sliced off at that view's top edge.
        .recipientSuggestionLayer()
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        .composeDraftTracking(currentHeaders) { draftChanges &+= 1 }
        .modifier(composerDrop)
        .modifier(draftSaving)
        .discardDraftQuestion(isPresented: $confirmingDiscard, discard: discardNow)
        .modifier(ComposerLinkDialogModifier(request: $editor.linkRequest, channel: editor.hostChannel))
        #else
        // iOS/iPadOS: a full-height sheet with the title + Close/Send in the navigation bar. Close
        // is leaving, so it keeps the draft; Discard is in the action bar (`docs/drafts.md`).
        NavigationStack {
            composerColumn { EmptyView() }
            .navigationTitle(title)
            .navigationBarTitleDisplayMode(.inline)
            // The standard Close symbol rather than the word, and Send as the bar's one prominent
            // action on its trailing side (Apple's Human Interface Guidelines, "Toolbars"). Both keep
            // their names for VoiceOver.
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button(role: .cancel) { cancel() } label: {
                        Label(L10n.action_close(), systemImage: "xmark")
                    }
                }
                ToolbarItem(placement: .confirmationAction) {
                    Button { prepareAndSend() } label: {
                        Label(L10n.action_send(), systemImage: "paperplane")
                    }
                    .buttonStyle(.borderedProminent)
                    .disabled(sendDisabled)
                }
            }
        }
        // Outside the navigation stack, so an open recipient list clears the editor's web view.
        // Inside it, on the scroll view, or on the field, WebKit draws over the list.
        .recipientSuggestionLayer()
        .modifier(composerDrop)
        .composeDraftTracking(currentHeaders) { draftChanges &+= 1 }
        .modifier(draftSaving)
        .discardDraftQuestion(isPresented: $confirmingDiscard, discard: discardNow)
        .modifier(ComposerLinkDialogModifier(request: $editor.linkRequest, channel: editor.hostChannel))
        #endif
    }

    /// A dropped file is resolved natively: web code sees a `File` with no path, so only the host
    /// can hand the bytes to Rust and list the file for removal. The same modifier on both hosts,
    /// so the macOS pane and the iPad sheet cannot come to behave differently.
    private var composerDrop: ComposerDropModifier {
        ComposerDropModifier(
            attach: attachFiles,
            droppedPictures: $droppedPictures,
            composerError: $composerError,
            editor: editor
        )
    }

    /// The composer as one column that scrolls as a whole (`docs/composer-layout.md`): `top` and the
    /// header, the notes, the files, then the message, which grows with its text. Nothing inside it
    /// scrolls on its own, so every file and every line is reached the same way.
    ///
    /// The editor is as tall as `composerEditorHeight` says: its content, or the room the visible
    /// part leaves under everything above it, whichever is more. It never takes `maxHeight:
    /// .infinity`, which inside a `ScrollView` is an unbounded web view.
    private func composerColumn<Top: View>(@ViewBuilder top: () -> Top) -> some View {
        let top = top()
        let editorTop = columnPadding + aboveEditorHeight + columnSpacing
        let room = visible.height - editorTop - columnPadding
        return ScrollView {
            VStack(alignment: .leading, spacing: columnSpacing) {
                VStack(alignment: .leading, spacing: columnSpacing) {
                    top
                    composerHeader
                    composerNotes
                    fileTiles
                }
                .onGeometryChange(for: CGFloat.self) { $0.size.height } action: {
                    aboveEditorHeight = $0
                }
                RichComposerWebView(editor: editor)
                    .frame(height: composerEditorHeight(contentPx: editor.contentHeight, roomPx: room))
                    .border(.quaternary)
            }
            .padding(columnPadding)
        }
        .scrollPosition($scrollPosition)
        .onScrollGeometryChange(for: ScrollWindow.self) { geometry in
            ScrollWindow(
                offset: geometry.contentOffset.y + geometry.contentInsets.top,
                height: geometry.containerSize.height - geometry.contentInsets.top
                    - geometry.contentInsets.bottom
            )
        } action: { _, window in
            visible = window
        }
        .onChange(of: editor.caret) { _, caret in
            guard let caret,
                  let target = composerCaretScroll(
                      offsetPx: visible.offset,
                      visiblePx: visible.height,
                      caretTopPx: editorTop + caret.top,
                      caretBottomPx: editorTop + caret.bottom
                  )
            else { return }
            scrollPosition.scrollTo(y: target)
        }
        #if !os(macOS)
        // Flicking the message down puts the keyboard away, which is the gesture people already
        // use to get back to the header without reaching for Done.
        .scrollDismissesKeyboard(.interactively)
        #endif
    }

    /// The From/recipient/subject fields, the action bar and the optional quote-style picker:
    /// everything above the editor.
    @ViewBuilder private var composerHeader: some View {
        // Every control in the composer is bound to its state (`$from`), never to a `Binding` built
        // from closures over `self`. Bound that way, the From pop-up button (macOS, two accounts or
        // more) keeps a closed composer in SwiftUI's graph, and with it the editor, its web view and
        // the WebContent process behind it. `ComposerReleaseTests` holds the line.
        FromAccountField(accounts: accounts, selection: $from)
            // Auto-swap: the signature follows the sender, because a work signature under a
            // personal address is the mistake this setting exists to prevent. Keyed on the
            // *resolved* account, so it doesn't fire when the binding settles on the same one.
            .onChange(of: resolvedFrom) { _, _ in fromAccountChanged() }
        // To carries the Cc/Bcc reveal: a chevron that points up while they are open. So the header
        // a message usually needs is From, To, Subject, as Gmail and Thunderbird draw it, and as
        // the Android composer does.
        RecipientField(
            label: L10n.compose_to(),
            text: $to,
            suggestionsFor: suggestionsFor,
            focusesOnAppear: focusesTo
        ) {
            Button {
                withAnimation { showsCcBcc.toggle() }
            } label: {
                Image(systemName: "chevron.down")
                    .rotationEffect(.degrees(showsCcBcc ? 180 : 0))
            }
            .buttonStyle(.plain)
            .accessibilityLabel(L10n.compose_show_cc_bcc())
        }
        if showsCcBcc {
            RecipientField(label: L10n.compose_cc(), text: $cc, suggestionsFor: suggestionsFor)
            RecipientField(label: L10n.compose_bcc(), text: $bcc, suggestionsFor: suggestionsFor)
        }
        // Editable whatever the composer is for. A reply and a forward open with the core's
        // derived `Re:`/`Fwd:` already in it, and renaming a thread here is what the user means by
        // editing it: the field's value is what gets sent.
        TextField(L10n.compose_subject(), text: $subject)
        actionBar
        if showsStylePicker {
            Picker(L10n.quote_style_label(), selection: $quoteStyle) {
                Text(L10n.quote_style_indented()).tag(QuoteStyleKind.indented)
                Text(L10n.quote_style_line_header()).tag(QuoteStyleKind.lineAndHeader)
            }
            .pickerStyle(.segmented)
            .onChange(of: quoteStyle) { _, style in
                editor.setQuoteStyle(ComposerQuote.token(style))
            }
            // A reopened quote keeps the style it was saved in, so the picker opens on that one.
            .onChange(of: editor.storedQuoteStyle) { _, style in
                if let style { quoteStyle = style }
            }
        }
    }

    /// The prepare-failure note and the draft hint, under the action row: never under the message,
    /// which grows as it is written (`docs/composer-layout.md`, rule 1).
    @ViewBuilder private var composerNotes: some View {
        if let composerError {
            Text(composerError)
                .font(.caption)
                .foregroundStyle(.red)
        }
        if let draftHint {
            Text(draftHint)
                .font(.caption)
                .foregroundStyle(draftHintFailed ? AnyShapeStyle(.red) : AnyShapeStyle(.secondary))
        }
    }

    func prepareAndSend() {
        composerError = nil
        editor.documentJSON { result in
            switch result {
            case .success(let documentJson):
                let submission = ComposerSubmission(
                    recipients: Recipients(to: to, cc: cc, bcc: bcc),
                    subject: subject,
                    documentJson: documentJson,
                    from: resolvedFrom,
                    composition: composition
                )
                if send(submission) {
                    draftReleased = true
                } else {
                    composerError = L10n.compose_prepare_error()
                }
            case .failure:
                composerError = L10n.compose_prepare_error()
            }
        }
    }
}

/// The column's scroll offset and the height of its visible part, keyboard and bars excluded.
private struct ScrollWindow: Equatable {
    let offset: CGFloat
    let height: CGFloat
}
