// The shared Apple SwiftUI shell: a macOS, iPhone, and iPad surface rendering the
// mailbox-list snapshot that the Rust core (mailcal-app, via the UniFFI `MailcalApp`
// object) drives. Dispatch an intent, the Rust app notifies the `Observer`, and SwiftUI
// pulls the immutable snapshot to render.

#if os(macOS)
import AppKit
#endif
import MailcalBindings
import SwiftUI

public struct ContentView: View {
    @State var model = MailboxModel()
    /// The swipe currently inside its undo window, and the rows it hides. Lives on the shell (not
    /// the list) so pushing the reading view on iPhone doesn't cancel the window.
    @State var swipeUndo = SwipeUndoController()
    @Environment(\.scenePhase) var scenePhase
    /// What the window is actually painted in right now, the app's own choice when it has one,
    /// the host's setting when it does not. Read rather than derived, because it is the only value
    /// that already accounts for both. iOS/iPadOS hands it to the Settings cover; see there.
    @Environment(\.colorScheme) var windowScheme
    @State var confirmingReset = false
    @State var compose: ComposeContext?
    /// The open draft's dirtiness, as reported by the hosted composer. macOS renders the composer in
    /// the detail column, so a click on another message can reach it, and must ask before dropping
    /// what the user has written. See Mailcal.ComposeDraft.swift.
    @State var draftProbe = ComposeDraftProbe()
    /// The message-open deferred while the "Discard draft?" prompt is up, run if the user discards.
    @State var pendingOpen: (() -> Void)?
    @State var confirmingDiscard = false
    @State var searchText = ""
    /// Settings is showing on this category, or `nil` when it is closed.
    ///
    /// One value rather than a `Bool` beside an optional category: two `@State` writes in a
    /// single action can be read by the sheet's content closure before the second one lands, and
    /// Settings then opens on whatever the first render held, which put the search horizon's
    /// "Change" on General instead of Accounts, intermittently.
    @State var settingsCategory: SettingsCategory?
    #if os(iOS)
    /// Whether the iPhone's accounts-and-folders drawer is showing. View state, not the core's:
    /// which accounts stand *expanded* is the core's and is persisted (`docs/folder-pane.md`
    /// rule 3), but whether the pane happens to be slid open is not something to restore.
    @State var sidebarOpen = false
    #endif
    @State var openedMessage: OpenedMessage?
    /// The contact whose detail is open. Fetched once when a row is picked (the lookup blocks on
    /// the core's runtime, so it runs off the main thread) rather than re-read on every render.
    @State var openedContact: OpenedContact?
    @State var expandedThreads: Set<String> = [] // keyed `account/threadId` → conversation sub-rows
    @State var accountToRemove: AccountRow? // the account a remove-confirmation is open for
    @State var sceneRestorationComplete = false
    @State var hasActivatedScene = false
    @State var hasLoggedSceneAppear = false
    /// The top-level surface to restore, as an `AppDestination` raw value. Deliberately a new key:
    /// the old boolean cannot express three destinations, and a stale `true` under a new meaning
    /// would restore the wrong screen. An unrecognised value restores mail.
    @SceneStorage("mailcal.destination") var sceneDestination = ""
    @SceneStorage("mailcal.selectedAccount") var sceneSelectedAccount = ""
    @SceneStorage("mailcal.selectedFolder") var sceneSelectedFolder = ""
    @SceneStorage("mailcal.openedAccount") var sceneOpenedAccount = ""
    @SceneStorage("mailcal.openedKey") var sceneOpenedKey = ""
    @SceneStorage("mailcal.viewMode") var sceneViewMode = ""
    #if DEBUG
    @State private var didAutoReply = false // one-shot guard for the MAILCAL_REPLY verification flag
    #endif
    @State var didShowcaseDrive = false // one-shot guards for the MAILCAL_SHOWCASE_SCREEN driver
    @State var didShowcaseReply = false

    public init() {}

    public var body: some View {
        Group {
            // First boot: welcome the user and ask the one question, ahead of setup and everything
            // else. The gate is the core's, not the UI's, `asked` also covers a returning user
            // upgrading into this version, who has accounts already but has never been asked. The
            // showcase and the demo report it settled (they have no store to record an answer in),
            // so no screenshot run ever sees this.
            if model.analyticsConsent?.asked == false {
                WelcomeView(
                    payloadPreview: { model.analyticsPayloadPreview() },
                    getStarted: { model.setAnalyticsConsent($0) }
                )
            } else if model.needsSetup {
                AccountSetupDetectView(
                    error: model.setupError,
                    signInMicrosoft: { hint in model.signInWithMicrosoft(loginHint: hint) },
                    signInGoogle: { hint in model.signInWithGoogle(loginHint: hint) },
                    signingIn: model.microsoftSigningIn,
                    googleSigningIn: model.googleSigningIn,
                    connecting: model.isConnecting,
                    submit: { imapHost, username, password, smtpHost, caldavURL, imapSecurity, smtpSecurity in
                        model.submitSetup(
                            imapHost: imapHost,
                            username: username,
                            password: password,
                            smtpHost: smtpHost,
                            caldavBaseUrl: caldavURL,
                            imapSecurity: imapSecurity,
                            smtpSecurity: smtpSecurity
                        )
                    },
                    submitJmap: { email, serverURL, password in
                        model.submitJmapSetup(
                            email: email,
                            serverURL: serverURL,
                            password: password
                        )
                    },
                    jmapOAuthAvailable: { email, serverURL in
                        await model.jmapOAuthAvailable(email: email, serverURL: serverURL)
                    },
                    signInJmap: { email, serverURL in
                        await model.signInWithJmap(email: email, serverURL: serverURL)
                    },
                    detect: { email in await model.detectSetup(email: email) },
                    // The first account, so the recommendation is offered here and only here.
                    onboarding: model
                )
            } else {
                mainView
            }
        }
        .onAppear {
            if !hasLoggedSceneAppear {
                hasLoggedSceneAppear = true
                logAppleLifecycle("scene appeared (\(applePlatformSummary()))")
            }
            model.start()
            restoreSceneIfPossible()
            #if os(iOS)
            // Schedule the background sync at every launch (invisible). The notification permission
            // is asked for separately, see `requestNotificationsIfSettled`.
            //
            // A showcase run does neither: the background pass would sync the developer's *stored*
            // accounts (the showcase connects none), and the permission alert would pop a system
            // dialog over the screenshot being taken.
            if !ShowcaseMode.isOn {
                scheduleBackgroundRefresh()
                requestNotificationsIfSettled()
            }
            #if DEBUG
            installDebugBackgroundTrigger()
            // On-device on-demand trigger (a `BGAppRefreshTask` can't be driven from the CLI and the
            // Darwin-notification trigger needs `notifyutil`, which a device lacks). Relaunch with
            //   devicectl … --environment-variables '{"MAILCAL_RUN_BGSYNC":"1"}'
            // to run one background pass (reusing the live core) and, via the DEBUG foreground
            // presenter, surface any new-mail banner even while foregrounded.
            if ProcessInfo.processInfo.environment["MAILCAL_RUN_BGSYNC"] == "1" {
                Task {
                    // Let connect() build the live core and seed the store first.
                    try? await Task.sleep(nanoseconds: 6_000_000_000)
                    await handleBackgroundRefresh()
                }
            }
            #endif
            #endif
            #if DEBUG
            // Verification: open the composer / calendar on launch (for simulator screenshots).
            if ProcessInfo.processInfo.environment["MAILCAL_COMPOSE"] == "1" { compose = .new }
            if ProcessInfo.processInfo.environment["MAILCAL_CALENDAR"] == "1" { showCalendar() }
            if ProcessInfo.processInfo.environment["MAILCAL_CONTACTS"] == "1" { showContacts() }
            #endif
            // Screenshot only: drive to the screen MAILCAL_SHOWCASE_SCREEN names. The settings and
            // add-account screens need no loaded rows, so try here as well as on the first row.
            showcaseSizeWindowIfNeeded()
            showcaseDriveIfNeeded()
        }
        .onChange(of: model.rows.count) { _, _ in
            restoreSceneIfPossible()
            autoOpenFirstIfRequested()
            showcaseDriveIfNeeded()
        }
        .onChange(of: model.accounts.count) { _, _ in restoreSceneIfPossible() }
        .onChange(of: model.folders.count) { _, _ in restoreSceneIfPossible() }
        .onChange(of: model.mode) { _, _ in restoreSceneIfPossible() }
        .onChange(of: model.destination) { _, _ in restoreSceneIfPossible() }
        .onChange(of: model.selectedAccount) { _, _ in restoreSceneIfPossible() }
        .onChange(of: model.selected) { _, _ in restoreSceneIfPossible() }
        .onChange(of: model.needsSetup) { _, _ in
            restoreSceneIfPossible()
            #if os(iOS)
            // The user just onboarded their first account, now it makes sense to ask.
            requestNotificationsIfSettled()
            #endif
        }
        #if os(iOS)
        // A returning user upgrading into this version has accounts already, so `needsSetup` never
        // changes and the line above never fires, but they still get the welcome screen, and the
        // system alert must not open on top of it. Waiting on the consent question here is what
        // sequences the two asks in that case.
        .onChange(of: model.analyticsConsent?.asked) { _, _ in requestNotificationsIfSettled() }
        #endif
        .onChange(of: model.reading?.key) { _, key in
            autoReplyIfRequested(readingKey: key)
            showcaseReplyIfNeeded(readingKey: key)
        }
        .onChange(of: scenePhase) { _, phase in handleScenePhaseChange(phase) }
        // The app-level light/dark setting (docs/settings.md → General). `nil` leaves the hierarchy
        // following the host, so a desktop switching scheme mid-session still reaches the app; a
        // forced scheme also reaches the sheets and covers presented from here, because they inherit
        // this environment.
        .preferredColorScheme(AppearanceMode.colorScheme(model.appearance))
    }

    /// DEBUG/verification only: with `MAILCAL_OPEN_FIRST=1` (or `MAILCAL_REPLY=1`), auto-open the
    /// first message once its row loads, so the reading view can be screenshotted without a tap.
    private func autoOpenFirstIfRequested() {
        #if DEBUG
        let env = ProcessInfo.processInfo.environment
        guard env["MAILCAL_OPEN_FIRST"] == "1" || env["MAILCAL_REPLY"] == "1",
              openedMessage == nil,
              let firstRow = model.rows.first,
              case let .flat(first) = firstRow
        else { return }
        open(first)
        #endif
    }

    /// DEBUG/verification only: with `MAILCAL_REPLY=1`, open the reply composer once the opened
    /// message's body has loaded (so the quoted original is available to seed).
    private func autoReplyIfRequested(readingKey: String?) {
        #if DEBUG
        guard !didAutoReply,
              ProcessInfo.processInfo.environment["MAILCAL_REPLY"] == "1",
              compose == nil, readingKey != nil, let opened = openedMessage
        else { return }
        didAutoReply = true // fire once per launch, so opening later messages doesn't re-reply
        beginReply(opened.account, opened.key, subject: opened.subject, all: false)
        #endif
    }

    #if os(iOS)
    @Environment(\.horizontalSizeClass) var hSize
    #endif

    private var mainView: some View {
        baseLayout
        .safeAreaInset(edge: .top) { offlineBanner }
        .safeAreaInset(edge: .top) { mailReauthBanner }
        .safeAreaInset(edge: .top) { signInExpiredBanner }
        .safeAreaInset(edge: .top) { accountNoticeBanner }
        .overlay(alignment: .top) { sendStatusBanner }
        .overlay(alignment: .bottom) { swipeUndoToast }
        .task(id: swipeUndo.pending?.id) { await runUndoWindow() }
        // An `alert`, not a `confirmationDialog`: iPadOS presents the latter as a popover, and a
        // popover DROPS the `.cancel`-role button, so this read as one destructive button with no
        // way out. See the remove-account alert in Mailcal.swift for the full note.
        .alert(
            L10n.reset_title(),
            isPresented: $confirmingReset
        ) {
            Button(L10n.reset_confirm(), role: .destructive) { model.reset() }
            Button(L10n.action_cancel(), role: .cancel) {}
        } message: {
            Text(L10n.reset_message())
        }
        // An `alert`, NOT a `confirmationDialog`: on iPad SwiftUI presents a confirmation dialog as
        // a popover, and a popover DROPS the `.cancel`-role button, the reasoning being that
        // tapping outside dismisses it. So this read as a single "Remove" button with no way out,
        // which is how App Review found it. An alert shows every button on every platform and size
        // class, and it is what Android (`AlertDialog` + `dismissButton`) and Windows
        // (`ContentDialog` + `CloseButtonText`) already do, so this is the odd one out being
        // brought into line, not a new pattern.
        .alert(
            L10n.remove_account_title(),
            isPresented: Binding(
                get: { accountToRemove != nil },
                set: { if !$0 { accountToRemove = nil } }
            ),
            presenting: accountToRemove
        ) { account in
            Button(L10n.action_remove(), role: .destructive) { removeAccount(account.id) }
            Button(L10n.action_cancel(), role: .cancel) {}
        } message: { account in
            Text(L10n.remove_account_message(email: account.email))
        }
        // The composer takes the whole window on iOS/iPadOS (a form-sheet modal leaves too little
        // room, especially on iPad). macOS no longer presents it at all: it renders INSIDE the
        // detail column, in place of the reading pane (macOSLayout), so the sidebar and
        // the message list stay live while you write. iPad has a reading pane and could take the
        // same treatment, but a sheet is idiomatic on touch, left as it is, deliberately, so the
        // two desktop clients land together.
        #if !os(macOS)
        .fullScreenCover(item: $compose) { composeContent($0) }
        #endif
        // An assistant asked to open a draft (docs/mcp.md). It arrives on the model from another
        // thread via `AgentComposerBridge`; the shell owns `compose`, so this is where it becomes
        // a composer. Deferred behind the discard prompt for the same reason a message click is:
        // an assistant must not be able to throw away what the user was writing.
        .onChange(of: model.pendingAgentDraft) { _, request in
            guard let request else { return }
            model.pendingAgentDraft = nil
            openDraft(request)
        }
        // Clicking another message with an unsent draft in the pane: Discard, or Keep editing.
        .modifier(DiscardDraftDialog(
            isPresented: $confirmingDiscard,
            compose: $compose,
            pendingOpen: $pendingOpen
        ))
        // Settings. One taxonomy (docs/settings.md), three chromes over the shared
        // SettingsCategoryDetail: macOS a sidebar+detail window, iPad a two-pane split, iPhone a
        // hub-and-spoke. iOS presents full-screen (like the composer): the iPad split needs a
        // regular-width container to show its two panes side by side, a form sheet is compact and
        // would collapse it to the iPhone hub, and iPhone reads as compact and shows the hub either
        // way. macOS keeps the sheet.
        #if os(macOS)
        .sheet(item: $settingsCategory, onDismiss: addAccountIfSettingsAskedFor) { category in
            SettingsView(
                model: model,
                close: { settingsCategory = nil },
                removeAccount: removeAccount,
                openOn: category
            )
        }
        #else
        .fullScreenCover(item: $settingsCategory, onDismiss: addAccountIfSettingsAskedFor) { category in
            SettingsHubView(
                model: model,
                close: { settingsCategory = nil },
                removeAccount: removeAccount,
                openOn: category
            )
            // The appearance again, on the cover's own content. `preferredColorScheme` travels UP
            // to a hosting controller, and on iOS/iPadOS a modal presentation gets its own, it
            // copies the presenter's scheme once, at presentation, and never again. Without this
            // the screen the Appearance picker LIVES ON keeps the old scheme while everything
            // behind it repaints, which reads as the setting not working. macOS needs no twin:
            // there a sheet shares the presenter's window and follows it live.
            //
            // The window's resolved scheme, NOT `AppearanceMode.colorScheme`, this must never go
            // back to `nil`. Releasing the preference leaves the cover's controller on the scheme
            // it was forced to resolve, so "Use system setting" would repaint it once and then
            // ignore the host for as long as the cover stays open. `windowScheme` is always an
            // explicit value and already carries both cases.
            .preferredColorScheme(windowScheme)
        }
        #endif
        // Adding another account while the app runs: the same setup form as first-run, as a
        // sheet. On success the model connects + stores it and dismisses; Cancel backs out.
        //
        // It cannot open while Settings is up, one modal does not present another, and the press
        // is silently dropped, so an offer pressed in there closes Settings and is opened by
        // `addAccountIfSettingsAskedFor` on the way out.
        .sheet(isPresented: $model.addingAccount) { addAccountSheet }
        // The device moved to a different time zone than the active one: prompt to switch.
        // The buttons drive the core state; the implicit-dismiss setter is ignored.
        .alert(
            L10n.tz_changed_title(),
            isPresented: Binding(
                get: { model.timezone?.pendingDevice != nil },
                set: { _ in }
            )
        ) {
            Button(L10n.action_update()) { model.acceptTimeZoneChange() }
            Button(L10n.tz_keep(zone: model.timezone?.active ?? L10n.tz_zone_current()), role: .cancel) {
                model.dismissTimeZoneChange()
            }
        } message: {
            Text(L10n.tz_changed_message(zone: model.timezone?.pendingDevice ?? L10n.tz_zone_new()))
        }
        // The calendar server stored the answer and then reported it could not tell the organiser:
        // offer to email it ourselves. The core owns both edges of this, it raises the question
        // and clears it the moment it is answered, so the presented binding only *reads* the
        // model, and the implicit-dismiss setter is ignored. `interactiveDismissDisabled` is what
        // keeps the two in step: a swipe-away would leave the core still holding a question the
        // user can no longer see or answer.
        .sheet(
            isPresented: Binding(get: { model.replyPrompt != nil }, set: { _ in })
        ) {
            if let prompt = model.replyPrompt {
                InvitationReplyPromptView(prompt: prompt) { send, remember in
                    model.answerReplyPrompt(send: send, remember: remember)
                }
                .interactiveDismissDisabled()
            }
        }
        // Same contract as the prompt above: the core owns the question, so the binding only
        // reads it and a swipe-away cannot orphan it.
        .sheet(
            isPresented: Binding(get: { model.unfiledCopy != nil }, set: { _ in })
        ) {
            if let unfiled = model.unfiledCopy {
                UnfiledCopyPromptView(
                    unfiled: unfiled,
                    onRetry: { model.retryUnfiledCopy() },
                    onDismiss: { model.dismissUnfiledCopy() }
                )
                .interactiveDismissDisabled()
            }
        }
    }

    /// Opens the setup sheet for an offer that was pressed inside Settings.
    ///
    /// Settings is itself a modal on both platforms, and a second modal presented over one is
    /// silently dropped, the button does nothing at all. So the press closes Settings and the
    /// intent is spent here, on the dismissal's own completion rather than after a guessed delay.
    private func addAccountIfSettingsAskedFor() {
        guard model.addAccountWhenSettingsCloses else { return }
        model.addAccountWhenSettingsCloses = false
        model.addingAccount = true
    }

    /// The add-another-account form, out of the modifier chain above.
    ///
    /// Not a style choice: SwiftUI's type checker budgets per expression, and a chain of a dozen
    /// modifiers carrying a call this size exhausts it, reporting the failure against whichever
    /// unrelated line it gave up on.
    @ViewBuilder
    private var addAccountSheet: some View {
        AccountSetupDetectView(
            error: model.setupError,
            cancel: {
                model.addingAccount = false
                model.setupError = nil
                model.setupStartEmail = ""
                model.setupStartOffer = nil
            },
            signInMicrosoft: { hint in model.signInWithMicrosoft(loginHint: hint) },
            signInGoogle: { hint in model.signInWithGoogle(loginHint: hint) },
            signingIn: model.microsoftSigningIn,
            googleSigningIn: model.googleSigningIn,
            connecting: model.isConnecting,
            submit: { imapHost, username, password, smtpHost, caldavURL, imapSecurity, smtpSecurity in
                model.submitSetup(
                    imapHost: imapHost,
                    username: username,
                    password: password,
                    smtpHost: smtpHost,
                    caldavBaseUrl: caldavURL,
                    imapSecurity: imapSecurity,
                    smtpSecurity: smtpSecurity
                )
            },
            submitJmap: { email, serverURL, password in
                model.submitJmapSetup(
                    email: email,
                    serverURL: serverURL,
                    password: password
                )
            },
            jmapOAuthAvailable: { email, serverURL in
                await model.jmapOAuthAvailable(email: email, serverURL: serverURL)
            },
            signInJmap: { email, serverURL in
                await model.signInWithJmap(email: email, serverURL: serverURL)
            },
            detect: { email in await model.detectSetup(email: email) },
            startEmail: model.setupStartEmail,
            startOffer: model.setupStartOffer,
            // Not the first account, so no card, but the accounts still to set up are not a
            // pitch, and are offered here too (`docs/onboarding.md`).
            onboarding: model,
            firstRun: false
        )
    }

}
