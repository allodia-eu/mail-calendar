import MailcalBindings
import SwiftUI

/// What a folder is CALLED on screen: our own word for a known folder, the server's name for
/// everything else (`docs/folder-pane.md` rule 12).
///
/// The server's name for a special folder is not a name the user chose, it is whatever their
/// provider stores, in whatever language and casing it likes: `INBOX` in capitals (the one name
/// IMAP mandates), `Deleted Items` from Exchange, `[Gmail]/Sent Mail`. Naming them ourselves is
/// what every mail client does, and it is also what makes the folder list follow the app's own
/// language rather than the server's.
///
/// A free function rather than a `ContentView` method because the sync-settings screen is a
/// different view over a different row type (`SyncFolderRow`), and a folder that is called two
/// things in one app is worse than one called something odd in both.
///
/// `.other` keeps the server name: the core collapses flagged, important and all-mail into that
/// one value, so no single word is honest for it.
func folderLabel(role: FolderRole?, name: String) -> String {
    switch role {
    case .inbox: L10n.folder_inbox()
    case .drafts: L10n.folder_drafts()
    case .sent: L10n.folder_sent()
    case .archive: L10n.folder_archive()
    case .junk: L10n.folder_junk()
    case .trash: L10n.folder_trash()
    case .other, nil: name
    }
}

/// The SF Symbol for a folder's special role, a plain folder for anything without one.
///
/// Keyed on the role the core resolves (RFC 6154 SPECIAL-USE / JMAP), never on the folder's name:
/// the name is whatever the server calls it, so a name test picks the wrong icon in six of the
/// seven shipped languages, and on any server whose folders were renamed.
func folderIcon(_ role: FolderRole?) -> String {
    switch role {
    case .inbox: "tray"
    case .drafts: "square.and.pencil"
    case .sent: "paperplane"
    case .archive: "archivebox"
    case .junk: "xmark.bin"
    case .trash: "trash"
    // A role we recognise but draw no distinct icon for (flagged / all / important), and every
    // ordinary custom folder.
    case .other, nil: "folder"
    }
}

/// One folder in the pane, identified by its account **and** its key.
///
/// A folder key is unique only within its account: every account calls its inbox `inbox`, so with
/// every account's tree on screen at once the pane holds several rows keyed `inbox`. Two rows
/// sharing an identity in one `List` are one row to SwiftUI, which drew the first account's unread
/// count on the second account's Inbox, the folders themselves were right, so nothing but the
/// number gave it away.
struct SidebarFolder: Identifiable {
    let account: String
    let folder: FolderRow

    var id: String { "\(account)/\(folder.key)" }
}

/// How wide the account row's expand/collapse target is: a pointer's width on macOS, a fingertip's
/// on iPadOS.
#if os(macOS)
let chevronTargetWidth: CGFloat = 16
#else
let chevronTargetWidth: CGFloat = 32
#endif

extension ContentView {
    /// The platform-adaptive base. macOS keeps its 3-pane split (sidebar | list | reading via
    /// HSplitView). iPad uses a 3-column NavigationSplitView (all columns visible at regular
    /// width, so the action-driven sidebar works without explicit navigation). iPhone uses a
    /// NavigationStack, the list with a leading menu for accounts/folders, pushing the reading
    /// view when a message opens. The shared banners/sheets/dialogs in `mainView` sit on top.
    @ViewBuilder var baseLayout: some View {
        #if os(macOS)
        macOSLayout
        #else
        if hSize == .compact { iPhoneLayout } else { iPadLayout }
        #endif
    }

    #if os(macOS)
    /// macOS keeps the desktop shell as a direct split view. `NavigationSplitView` can collapse
    /// or overlay columns on macOS in ways that are useful for document navigation but wrong for
    /// this mail layout, where sidebar, list, and reading pane must all remain visible.
    ///
    /// **The split's two panes must survive a destination change**, that is what keeps the widths
    /// you drag. AppKit restores an autosaved layout when a split view is *set up*
    /// and never again, so a split whose panes are torn down under it loses the divider and then
    /// saves the collapsed result over the good one. Switching Mail → Calendar → Mail used to take
    /// the sidebar to its `maxWidth` and the list to its `minWidth` for exactly that reason.
    ///
    /// Two things together fix it, and each was necessary:
    ///
    /// 1. Destinations that show more than one pane put theirs in a **nested** split
    ///    (`destinationPanes`), so this split's pane *count* no longer changes with the
    ///    destination, and each nested split is genuinely created when you enter its
    ///    destination, which is the moment AppKit restores.
    /// 2. The second pane is wrapped in a container that outlives the branch change. A `switch`
    ///    written directly here is a `_ConditionalContent`, and SwiftUI rebuilds that subtree
    ///    whenever the branch changes, **even between two branches of the same shape** (Mail →
    ///    Contacts → Mail, both nested splits, still snapped the sidebar to 320). The `ZStack`
    ///    persists, so the swap happens *inside* a pane the split never sees change.
    var macOSLayout: some View {
        HSplitView {
            sidebarList(showsCalendarAndContacts: true)
                .frame(minWidth: 220, idealWidth: 240, maxWidth: 320)
                .background(SplitViewAutosave(name: AppPrefs.autosaveName("AllodiaMailMacSidebarV3")))

            ZStack { destinationPanes }
        }
        .frame(minWidth: 1_080, minHeight: 520)
    }

    /// Everything to the right of the sidebar. Each destination owns its own split and its own
    /// autosave name, so the widths a user sets in the mailbox and in contacts are separate
    /// choices that both survive leaving and coming back, see `macOSLayout` for why the nesting
    /// is load-bearing.
    @ViewBuilder private var destinationPanes: some View {
        switch model.destination {
        case .calendar:
            // One pane, the grid takes the whole width beside the sidebar, so there is no
            // divider here to remember.
            calendarDetail
                .frame(minWidth: 620)
        case .contacts:
            // List | detail, like mail, a list of people is something you pick one of, where
            // the calendar is one wide grid with nothing to select beside it.
            HSplitView {
                contactsList
                    .frame(minWidth: 300, idealWidth: 360, maxWidth: 480)
                    .background(
                        SplitViewAutosave(name: AppPrefs.autosaveName("AllodiaMailMacContactsV3"))
                    )
                contactsDetailPane
                    .frame(minWidth: 420, idealWidth: 620)
            }
        case .mail:
            HSplitView {
                messageList
                    .frame(minWidth: 420, idealWidth: 540, maxWidth: 720)
                    .background(
                        SplitViewAutosave(name: AppPrefs.autosaveName("AllodiaMailMacMailV3"))
                    )
                // The second column is the reading pane, or, while a draft is open, the composer
                // in its place. Writing a message no longer blacks out the mailbox
                // behind a sheet: the sidebar and the list stay live, and clicking another message
                // asks before it drops the draft (openGuardingDraft). Swapping the two keeps this
                // split at two panes, so opening a draft doesn't disturb the divider either.
                detailColumn
                    .frame(minWidth: 420, idealWidth: 760)
            }
        }
    }

    /// The macOS detail column: the composer when one is open, else the reading pane.
    @ViewBuilder var detailColumn: some View {
        if let compose {
            composeContent(compose)
        } else {
            readingPane
        }
    }
    #endif

    /// The accounts / folders / settings sidebar.
    ///
    /// `showsCalendarAndContacts` is what a platform answers with where its other surfaces live.
    /// The desktop and the iPad columns reach them from here, because the pane is the only
    /// navigation they have; the iPhone reaches them from its tab bar, so listing them here too
    /// would offer the same two destinations twice. Settings is on the pane either way, it has no
    /// tab, and it must not be something the user has to remember a gesture to find.
    func sidebarList(showsCalendarAndContacts: Bool) -> some View {
        List {
            Section(L10n.sidebar_accounts()) {
                sidebarRow(
                    title: L10n.sidebar_all_inboxes(),
                    icon: "tray.full",
                    selected: model.destination == .mail && model.selectedAccount == nil,
                    unread: model.unifiedUnread
                ) { selectAccount(nil) }
                ForEach(model.accounts, id: \.id) { account in
                    HStack(spacing: 0) {
                        // The chevron is its own control: opening a tree is not navigating, so it
                        // must not move the selection (`docs/folder-pane.md`). The core owns the
                        // state and persists it, nothing here is remembered locally, which is
                        // what makes it survive a relaunch and agree with the other platforms.
                        Button {
                            model.setAccountExpanded(account.id, !account.expanded)
                        } label: {
                            Image(systemName: account.expanded ? "chevron.down" : "chevron.right")
                                .font(.caption)
                                // A pointer hits the glyph; a finger needs the area around it, and
                                // a near miss here is not a no-op, it lands on the account row and
                                // navigates. The height stays inside the row so the target grows
                                // without the row growing with it.
                                .frame(width: chevronTargetWidth, height: 28)
                                .contentShape(Rectangle())
                        }
                        .buttonStyle(.plain)
                        .accessibilityLabel(
                            account.expanded
                                ? L10n.a11y_collapse_account()
                                : L10n.a11y_expand_account()
                        )
                        sidebarRow(
                            title: account.email,
                            icon: "person.crop.circle",
                            selected: model.destination == .mail
                                && model.selectedAccount == account.id
                        ) { selectAccount(account.id) }
                    }
                    // Right-click an account to remove it (with a confirmation).
                    .contextMenu {
                        Button(L10n.action_remove_account(), role: .destructive) {
                            accountToRemove = account
                        }
                    }
                    // A warning badge when this account's server couldn't be reached on its
                    // last sync (while the device is online), a per-account outage, distinct
                    // from the device-wide offline banner.
                    .overlay(alignment: .trailing) {
                        if model.isAccountUnreachable(account.id) {
                            Image(systemName: "exclamationmark.triangle.fill")
                                .foregroundStyle(.orange)
                                .help(L10n.connectivity_account_unreachable())
                                .padding(.trailing, 10)
                        }
                    }
                    // Every expanded account's folders show indented beneath it, not just the
                    // selected one's, which is why the tree no longer empties when the user picks
                    // All Inboxes or the account next door.
                    if model.destination == .mail && account.expanded {
                        sidebarRow(
                            title: L10n.sidebar_all_mail(),
                            icon: "tray.full",
                            selected: model.selectedAccount == account.id && model.selected == nil,
                            indent: true
                        ) { selectAccount(account.id) }
                        ForEach(model.folderRows(for: account.id)) { row in
                            let folder = row.folder
                            sidebarRow(
                                title: folderLabel(role: folder.role, name: folder.name),
                                icon: folderIcon(folder.role),
                                selected: model.selectedAccount == account.id
                                    && model.selected == folder.key,
                                indent: true,
                                unread: folder.unread
                            ) { selectFolder(in: account.id, key: folder.key) }
                        }
                    }
                }
                sidebarRow(title: L10n.action_add_account(), icon: "plus.circle", selected: false) {
                    model.setupError = nil
                    model.addingAccount = true
                }
            }
            if showsCalendarAndContacts {
                Section(L10n.nav_calendar()) {
                    sidebarRow(
                        title: L10n.nav_calendar(),
                        icon: "calendar",
                        selected: model.destination == .calendar
                    ) { showCalendar() }
                }
                Section(L10n.nav_contacts()) {
                    sidebarRow(
                        title: L10n.nav_contacts(),
                        icon: "person.2",
                        selected: model.destination == .contacts
                    ) { showContacts() }
                }
            }
            Section(L10n.settings_title()) {
                // ⌘, lives here, on the app's one Settings affordance. It used to hang off a second
                // gear in the message-list header; that button is gone, so the shortcut moved rather
                // than leaving the standard macOS key with nothing to open.
                sidebarRow(title: L10n.nav_settings(), icon: "gearshape", selected: false) {
                    settingsCategory = .general
                }
                #if os(macOS)
                .keyboardShortcut(",", modifiers: .command)
                #endif
            }
        }
        .listStyle(.sidebar)
        .frame(minWidth: 180)
        // The window title. The product is "Allodia Mail & Calendar", never bare "Allodia"
        // (AGENTS.md → "Brand & voice"), and this overrides the WindowGroup's own title.
        .navigationTitle(L10n.app_title())
        #if os(macOS)
        // Set, then hidden, the two are not the same thing. The title still names the window to
        // the OS, which is what the Window menu, ⌘-Tab and Mission Control read; what goes is the
        // copy of it drawn over the sidebar, which spent the top of the accounts column telling the
        // user the name of the app they had just opened. Dropping `navigationTitle` instead would
        // have taken the window's name with it and left the Window menu listing "Untitled".
        .toolbar(removing: .title)
        #endif
    }

    #if os(iOS)
    /// The iPhone message list: native chrome, a `.searchable` bar and a `.plain` list, with
    /// the view-mode/refresh/reset controls in the toolbar menu (the desktop `messageList`'s
    /// fixed-width header/footer don't fit a phone).
    var compactMessageList: some View {
        VStack(spacing: 0) {
            SearchHorizonStrip(horizon: model.searchHorizon) { settingsCategory = .accounts }
            // The count and the batched actions, over the rows they describe. Nothing selected
            // draws nothing, so the list keeps its full height the rest of the time.
            selectionBar
            selectionBehaviour(
                List {
                    let rows = visibleRows
                    ForEach(rows, id: \.rowID) { row in
                        rowView(row)
                            .listRowBackground(rowHighlight(row))
                            .onAppear {
                                if row.rowID == rows.last?.rowID {
                                    Task { @MainActor in model.showMore() }
                                }
                            }
                    }
                }
                .listStyle(.plain)
                .searchable(text: $searchText, prompt: Text(L10n.search_placeholder()))
                .onChange(of: searchText) { _, query in model.search(query) }
            )
            // Below the list, and outside it. As the list's first *row* it pushed every message
            // down when a background sync began and back up when it ended; as a strip under the
            // list it neither moves the rows nor scrolls away with them.
            //
            // The phone has no footer to put the background hint in, so it shares this strip:
            // the same edge, without the bar. Both can be true at once (an awaited download while
            // a poll tick catches another account up); the bar is the one the user is waiting on,
            // so it wins the strip.
            if let progress = model.syncProgress, progress.active {
                Divider()
                HStack(spacing: 8) {
                    ProgressView().controlSize(.small)
                    Text(syncProgressText(progress)).font(.caption).foregroundStyle(.secondary)
                    Spacer()
                }
                .padding(.horizontal, 16)
                .padding(.vertical, 8)
            } else if let hint = syncHintText(model.syncProgress) {
                Divider()
                HStack {
                    Text(hint).font(.caption).foregroundStyle(.secondary)
                    Spacer()
                }
                .padding(.horizontal, 16)
                .padding(.vertical, 8)
            }
        }
    }

    /// View-mode / refresh / reset, the message-list actions, shared by the iPhone and iPad
    /// toolbars.
    var messageListMenu: some View {
        Menu {
            Picker(
                L10n.view_label(),
                selection: Binding(get: { model.mode }, set: { setViewMode($0) })
            ) {
                Text(L10n.view_flat()).tag(ViewMode.flat)
                Text(L10n.view_threaded()).tag(ViewMode.threaded)
            }
            Button { model.refresh() } label: {
                Label(L10n.action_refresh(), systemImage: "arrow.clockwise")
            }
            Button(role: .destructive) { confirmingReset = true } label: {
                Label(L10n.action_reset_database(), systemImage: "trash")
            }
        } label: { Image(systemName: "ellipsis.circle") }
    }

    #endif

    /// The current scope's title: the unified "All Inboxes" (no account selected), else the
    /// selected folder's name or the account's "All Mail".
    var currentFolderName: String {
        if model.selectedAccount == nil { return L10n.sidebar_all_inboxes() }
        guard let key = model.selected else { return L10n.sidebar_all_mail() }
        guard let folder = model.folders.first(where: { $0.key == key }) else {
            return L10n.folder_fallback()
        }
        return folderLabel(role: folder.role, name: folder.name)
    }

    @ViewBuilder
    func sidebarRow(
        title: String,
        icon: String,
        selected: Bool,
        indent: Bool = false,
        unread: UInt32 = 0,
        action: @escaping () -> Void
    ) -> some View {
        Button(action: action) {
            HStack {
                // One line, truncated at the end. An account address is as long as it is, and a
                // pane narrow enough to wrap one (every iPad column) turns each account into a
                // two-line row with its domain orphaned underneath, `eva.jansen@example.c` over
                // `om`. Rule 11 already says the row says in full what it shortened.
                Label(title, systemImage: icon)
                    .lineLimit(1)
                    .truncationMode(.tail)
                Spacer()
                // Never at zero, which deliberately also covers "this provider reports no
                // count" (Gmail today): both mean there is nothing truthful to show, and a 0
                // would claim we had looked (`docs/folder-pane.md`). The number alone reads as a
                // list position aloud, so VoiceOver gets the sentence instead.
                if unread > 0 {
                    Text("\(unread)")
                        .foregroundStyle(Color.accentColor)
                        .accessibilityLabel(L10n.a11y_unread_count(count: Int(unread)))
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(.leading, indent ? 16 : 0)
            // The whole row is the target, not just the words on it. A `Spacer` is not
            // hit-testable, so the gap the count sits beside swallowed every click landing in
            // the middle of a row, the wider the pane, the more of the row was dead.
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .listRowBackground(selected ? Color.accentColor.opacity(0.2) : Color.clear)
        #if os(macOS)
        // Truncation is unavoidable at some pane width, so the row says in full what it had to
        // shorten, an address clipped mid-domain is precisely the row the user needed to read
        // (`docs/folder-pane.md` rule 11, as the Windows pane does on every row).
        .help(title)
        #endif
    }
}
