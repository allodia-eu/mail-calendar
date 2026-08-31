// The rich new-message composer for the shared Apple client. The editor document is
// trusted bundled code loaded in a dedicated WKWebView; the body output is still produced
// by shared Rust (`submitRichMail` validates and renders before queuing the send).

import Foundation
import MailcalBindings
import SwiftUI
import WebKit

private struct RichComposerWebView: PlatformViewRepresentable {
    let editor: RichComposerEditor

    #if os(macOS)
    func makeNSView(context: Context) -> WKWebView { editor.webView }
    func updateNSView(_ nsView: WKWebView, context: Context) {}
    #else
    func makeUIView(context: Context) -> WKWebView { editor.webView }
    func updateUIView(_ uiView: WKWebView, context: Context) {}
    #endif
}

/// The least room the editor keeps once the header and the keyboard have taken theirs.
///
/// Below this the message being written stops being visible at all, which is worse than making the
/// header scroll, so past this point the column scrolls instead of shrinking further.
///
/// Not a taste value: it has to clear the wrapped toolbar plus the editor's own `min-height: 180px`
/// from the shared bundle. The host pins the document against scrolling (`RichComposerEditor`), so
/// a frame shorter than its content would overflow with nothing able to reach it.
private let minimumEditorHeight: CGFloat = 280

/// What the rich composer is for. Every mode now exposes editable To/Cc/Bcc fields, a
/// reply and reply-all open with To/Cc pre-filled from the core (`replyRecipients`), a
/// forward and new message open empty, so the user can adjust any recipient. Only a new
/// message edits the Subject (reply/forward derive `Re:`/`Fwd:` in the core). Every mode
/// shares the one hardened editor host.
enum RichComposeMode {
    case new
    case reply
    case replyAll
    case forward

    var showsSubject: Bool { self == .new }
}

/// Everything the composer needs to seed, swap, and override signatures, the library to list, and
/// the two lookups the core answers (the account's signature for this mode, and one by id). Passed
/// as a value rather than the model so `RichComposeView` stays free of it; `nil` turns the feature
/// off entirely, which is what a preview or a screenshot run wants.
struct ComposerSignatures {
    /// The library, for the picker.
    let library: [SignatureRow]
    /// The signature `account` uses in `slot`, or `nil` when that slot is unassigned.
    let forAccount: (String, SignatureSlotKind) -> SignatureBody?
    /// One signature by id, the per-message override.
    let byId: (String) -> SignatureBody?
}

/// What this one message's signature should be. `nil` (the initial state) means **follow the
/// account**: the signature re-resolves whenever the From dropdown changes, which is what a user
/// who never touched the picker expects, their work signature when sending from work.
///
/// Once they pick explicitly, that choice sticks even across a From change: they chose it *for this
/// message*, and silently replacing it would undo a deliberate act. (Outlook re-swaps regardless,
/// which is its most complained-about composer behaviour.)
/// Spelled `noSignature` rather than `none`: as an `Optional<SignatureChoice>`, which is how it is
/// held, since `nil` means "follow the account", a case called `none` would collide with
/// `Optional.none` at every `switch` and pattern match.
///
/// Not `private`: RichComposerView.Signature.swift matches on it too.
enum SignatureChoice: Equatable {
    /// No signature on this message.
    case noSignature
    /// This specific signature, by id.
    case signature(String)
}

/// Rich composer for a new message, reply, reply-all, or forward: the shared local editor
/// bundle plus the editable From/To/Cc/Bcc header fields (and Subject for a new message). `send`
/// receives the entered [`Recipients`], the Subject (used only for a new message), the
/// rendered document JSON, the attachments, and the id of the account picked in the From
/// dropdown; the parent routes it to the matching `submitRich*` call. Reply and reply-all pass
/// `initialTo`/`initialCc` to pre-fill the derived recipients, and every mode passes the
/// `accounts` to choose a sender from plus the `initialFrom` it opens on.
struct RichComposeView: View {
    let title: String
    var mode: RichComposeMode = .new
    let accounts: [AccountRow]
    let send: (Recipients, String, String, [ComposerFileAttachment], String?) -> Bool
    let cancel: () -> Void
    /// Ranked address suggestions for a partially-typed recipient, answered by the core from synced
    /// contacts **and** from people the user has written to before, so it works on an account with
    /// no address book at all. `nil` disables autosuggest (a preview or a screenshot run).
    var suggestionsFor: ((String) async -> [RecipientMatch])?
    /// The shell's handle on this draft, so it can ask whether anything has been written before it
    /// hands the detail column to another message (macOS's inline composer). `nil` on
    /// iOS/iPadOS, where the composer is a full-screen cover and no row is clickable behind it.
    var probe: ComposeDraftProbe?
    /// The signature library + lookups, or `nil` to disable signatures for this composer.
    var signatures: ComposerSignatures?
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
    @State private var to: String
    @State private var cc: String
    @State private var bcc: String
    /// Whether the Cc/Bcc rows are revealed. Collapsed unless the caller pre-filled one.
    @State private var showsCcBcc: Bool
    @State private var subject: String
    @State private var prepareError = false
    @State private var quoteStyle: QuoteStyleKind
    /// Not `private`: RichComposerView.Attachments.swift's `attachmentList` and
    /// `chooseAttachments` read and set it too.
    @State var attachments: [PickedAttachment] = []
    /// Whether the To field takes the caret when the composer appears, the other half of
    /// `opensInBody`, so exactly one of the two is focused.
    private let focusesTo: Bool
    #if !os(macOS)
    /// The header's measured height, so the editor below it can take what is left of the screen.
    @State private var headerHeight: CGFloat = 0
    #endif

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
        quote: String? = nil,
        quoteStyle: QuoteStyleKind = .indented,
        quoteStylePerMessage: Bool = false,
        probe: ComposeDraftProbe? = nil,
        suggestionsFor: ((String) async -> [RecipientMatch])? = nil,
        signatures: ComposerSignatures? = nil,
        send: @escaping (Recipients, String, String, [ComposerFileAttachment], String?) -> Bool,
        cancel: @escaping () -> Void
    ) {
        self.title = title
        self.mode = mode
        self.accounts = accounts
        self.send = send
        self.cancel = cancel
        self.probe = probe
        self.suggestionsFor = suggestionsFor
        self.signatures = signatures
        self.showsStylePicker = ComposerQuote.showsStylePicker(
            hasQuote: quote != nil,
            perMessage: quoteStylePerMessage
        )
        self.focusesTo = !Self.opensInBody(mode: mode, to: initialTo)
        let editor = RichComposerEditor()
        editor.pendingQuote = quote
        editor.pendingPlainBody = initialBody.isEmpty ? nil : initialBody
        // The opening signature is resolved here, not in `onAppear`: it has to be injected before
        // the editor snapshots its "nothing written yet" seed, or the composer opens dirty. The
        // account is resolved the same way `resolvedFrom` does below, the From dropdown and the
        // signature must agree about who is sending.
        let opening = accounts.contains { $0.id == initialFrom } ? initialFrom : accounts.first?.id
        editor.pendingSignature = opening
            .flatMap { signatures?.forAccount($0, signatureSlot(for: mode)) }
            .flatMap(Self.signatureSeed)
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
        _quoteStyle = State(initialValue: quoteStyle)
    }

    /// Whether this composer opens with the caret in the message body rather than in To.
    ///
    /// The two are exclusive, and every client decides it the same way (docs/contacts.md §4).
    static func opensInBody(mode: RichComposeMode, to: String) -> Bool {
        mode != .new || !to.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
    }

    private var sendDisabled: Bool {
        to.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
    }

    /// The account the From field shows *and* the one the send goes out as. `from` can name no
    /// configured account, the composer may be opened before the first snapshot lands, leaving
    /// the caller's `initialFrom` nil, and a Picker whose selection matches no tag renders
    /// blank. Resolving here keeps the visible sender and the submitted `from` the same account,
    /// which is the whole point of the picker.
    ///
    /// Not `private`: RichComposerView.Signature.swift's `accountSignature` reads it too.
    var resolvedFrom: String? {
        accounts.contains { $0.id == from } ? from : accounts.first?.id
    }

    private var fromBinding: Binding<String?> {
        Binding(get: { resolvedFrom }, set: { from = $0 })
    }

    var body: some View {
        #if os(macOS)
        // macOS: the composer fills the detail column, in place of the reading pane.
        // Deliberately NOT the fixed 620x560 it carried as a window-sheet, the column is
        // user-resizable (the list|reading splitter), so a fixed frame would sit stranded in it and
        // clip its own editor at a narrow width. Everything stretches; the editor takes the slack.
        // Send and Cancel live in the action bar above the editor now (see `actionBar`), not in a
        // row under it, so the message's actions sit together instead of straddling the body.
        VStack(alignment: .leading, spacing: 12) {
            Text(title).font(.headline)
            composerHeader
            RichComposerWebView(editor: editor)
                .frame(minHeight: 240, maxHeight: .infinity)
                .border(.quaternary)
            composerFooter
        }
        // On the composer's outermost view, so an open recipient list clears the editor's web
        // view. Nearer the field it is sliced off at that view's top edge.
        .recipientSuggestionLayer()
        .padding(16)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        .composeDraftTracking(probe: probe, editor: editor, to: to, cc: cc, bcc: bcc, subject: subject, attachments: attachments.count)
        #else
        // iOS/iPadOS: a full-height sheet with the title + Cancel/Send in the navigation bar.
        NavigationStack {
            phoneComposerBody
            .navigationTitle(title)
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button(L10n.action_cancel(), role: .cancel) { cancel() }
                }
                ToolbarItem(placement: .confirmationAction) {
                    Button(L10n.action_send()) { prepareAndSend() }.disabled(sendDisabled)
                }
            }
        }
        // Outside the navigation stack, so an open recipient list clears the editor's web view.
        // Inside it, on the scroll view, or on the field, WebKit draws over the list.
        .recipientSuggestionLayer()
        #endif
    }


    #if !os(macOS)
    /// The touch composer, in one scroll.
    ///
    /// The column cannot simply be laid out and left: the header grows without a ceiling (wrapped
    /// recipient pills, an open suggestion list) while the keyboard takes roughly a third of the
    /// screen, and the editor underneath has a floor it will not go below. Something has to give,
    /// and without a scroll it is whatever sits at the bottom, so the toolbar clipped mid-row and
    /// the body could not be reached at all.
    ///
    /// The editor is given the height that is actually left rather than `maxHeight: .infinity`,
    /// which inside a `ScrollView` means an unbounded web view that never stops growing.
    @ViewBuilder private var phoneComposerBody: some View {
        GeometryReader { proxy in
            ScrollView {
                VStack(alignment: .leading, spacing: 12) {
                    composerHeader
                        .onGeometryChange(for: CGFloat.self) { $0.size.height } action: {
                            headerHeight = $0
                        }
                    RichComposerWebView(editor: editor)
                        .frame(height: max(minimumEditorHeight, proxy.size.height - headerHeight - 44))
                        .border(.quaternary)
                    composerFooter
                }
                .padding(16)
            }
            // Flicking the message down puts the keyboard away, which is the gesture people already
            // use to get back to the header without reaching for Done.
            .scrollDismissesKeyboard(.interactively)
        }
    }
    #endif

    /// The From/recipient/subject fields, the action bar and the optional quote-style picker:
    /// everything above the editor.
    @ViewBuilder private var composerHeader: some View {
        FromAccountField(accounts: accounts, selection: fromBinding)
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
        if mode.showsSubject {
            TextField(L10n.compose_subject(), text: $subject)
        }
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
        }
    }

    /// The attachment list and the prepare-failure note, everything below the editor.
    @ViewBuilder private var composerFooter: some View {
        attachmentList
        if prepareError {
            Text(L10n.compose_prepare_error())
                .font(.caption)
                .foregroundStyle(.red)
        }
    }

    /// The composer's action bar, above the editor, Outlook's arrangement, and the reason the
    /// signature is *here* rather than as a labelled row among From/To/Cc: it is an action you take
    /// on the message, not a field you address it with. Attaching a file is the same kind of thing,
    /// so it moved out of its own row below the editor and joined it.
    ///
    /// Send and Discard lead the bar on macOS, where the composer is an inline pane with no window
    /// chrome of its own. On iOS they stay in the navigation bar, the platform puts confirm/cancel
    /// there, and repeating them in the body would be two Sends on one screen.
    @ViewBuilder private var actionBar: some View {
        HStack(spacing: 10) {
            #if os(macOS)
            Button {
                prepareAndSend()
            } label: {
                Label(L10n.action_send(), systemImage: "paperplane")
            }
            .buttonStyle(.borderedProminent)
            .controlSize(.small)
            .disabled(sendDisabled)
            Button(role: .cancel) {
                cancel()
            } label: {
                Label(L10n.action_cancel(), systemImage: "trash")
            }
            .buttonStyle(.bordered)
            .controlSize(.small)
            Divider().frame(height: 16)
            #endif
            Button {
                chooseAttachments()
            } label: {
                Label(L10n.action_attach(), systemImage: "paperclip")
            }
            .buttonStyle(.bordered)
            .controlSize(.small)
            if showsSignaturePicker {
                signatureMenu
            }
            Spacer()
        }
    }

    private func prepareAndSend() {
        prepareError = false
        editor.documentJSON { result in
            switch result {
            case .success(let documentJson):
                let recipients = Recipients(to: to, cc: cc, bcc: bcc)
                let files = attachments.map(\.composerFile)
                if !send(recipients, subject, documentJson, files, resolvedFrom) {
                    prepareError = true
                }
            case .failure:
                prepareError = true
            }
        }
    }
}
