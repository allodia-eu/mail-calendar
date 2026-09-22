// The accounts-and-folders pane on every Apple platform: the All Accounts group, each account's
// tree, and the naming and icon mappings the core cannot supply.
//
// Split from Mailcal.Layout.swift, which holds the layouts the pane sits in, so both stay under
// the repo line limit. The rules are `docs/folder-pane.md`.

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

/// What the message list's header calls the scope on screen.
///
/// A free function over the three values it needs, rather than a `ContentView` property, so the
/// answer can be asserted without a window: it is one of the two places a folder is named, and
/// the pane is the other (`docs/folder-pane.md` rule 13).
func listTitle(selectedAccount: String?, selectedFolder: String?, folders: [FolderRow]) -> String {
    // The unified list is the All Accounts group's **Inbox** row, so it is called what that row
    // is called. Naming the group here instead would put a word on the header that no row in
    // the pane carries.
    guard selectedAccount != nil else { return L10n.folder_inbox() }
    guard let key = selectedFolder else { return L10n.sidebar_all_mail() }
    guard let folder = folders.first(where: { $0.key == key }) else {
        // A key with no row behind it: the folder list has moved on (a rename, a sync), and the
        // header would otherwise name a folder that is no longer there.
        return L10n.folder_fallback()
    }
    return folderLabel(role: folder.role, name: folder.name)
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

/// One step of the pane's indent: what an account's own rows are moved by, and what each level
/// of folders inside a folder adds again.
let indentWidth: CGFloat = 16

extension ContentView {
    /// Whether the mailbox is the surface on screen, which is what decides whether a **mail** row
    /// carries the highlight.
    ///
    /// Only the highlight: which rows are drawn is the trees' own expansion and nothing else
    /// (`docs/folder-pane.md`, rule 2). Without this the pane would light a folder while the
    /// calendar is showing, next to the lit Calendar row beneath it.
    var showingMail: Bool { model.destination == .mail }

    /// The accounts / folders / settings sidebar: every account's tree scrolling under the
    /// **Accounts** heading, with the other destinations pinned beneath it.
    ///
    /// `showsCalendarAndContacts` belongs to those, and
    /// ``sidebarDestinations(showsCalendarAndContacts:)`` says what it answers.
    func sidebarList(showsCalendarAndContacts: Bool) -> some View {
        List {
            Section(L10n.sidebar_accounts()) {
                outboxRow
                allAccountsGroup
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
                            selected: showingMail && model.selectedAccount == account.id
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
                    // selected one's, and whatever surface is on screen: the pane is furniture,
                    // so looking at the calendar is not a reason for a tree to empty
                    // (`docs/folder-pane.md`, rule 2). The rows only stop being *highlighted*,
                    // because what the user is in is the calendar, not a folder.
                    if account.expanded {
                        HStack(spacing: 0) {
                            disclosureSlot
                            sidebarRow(
                                title: L10n.sidebar_all_mail(),
                                icon: "tray.full",
                                selected: showingMail
                                    && model.selectedAccount == account.id
                                    && model.selected == nil
                            ) { selectAccount(account.id) }
                        }
                        .padding(.leading, indentWidth)
                        ForEach(model.folderRows(for: account.id)) { row in
                            let folder = row.folder
                            HStack(spacing: 0) {
                                folderDisclosure(in: account.id, folder: folder)
                                sidebarRow(
                                    title: folderLabel(role: folder.role, name: folder.name),
                                    icon: folderIcon(folder.role),
                                    selected: showingMail
                                        && model.selectedAccount == account.id
                                        && model.selected == folder.key,
                                    unread: folder.unread
                                ) { selectFolder(in: account.id, key: folder.key) }
                            }
                            // One step per level, plus the account's own. The indent is on the
                            // stack rather than inside the row so the chevrons line up down a
                            // branch, the way the account chevrons line up down the pane.
                            .padding(.leading, indentWidth * CGFloat(folder.depth + 1))
                        }
                    }
                }
                sidebarRow(title: L10n.action_add_account(), icon: "plus.circle", selected: false) {
                    model.setupError = nil
                    model.addingAccount = true
                }
            }
        }
        .listStyle(.sidebar)
        .frame(minWidth: 180)
        // The other destinations sit under the tree and out of its scroll, never as one more
        // section at the end of the list: an account with a few dozen folders fills the pane, and
        // a row that scrolls away with them is a destination the user has to scroll back up the
        // whole folder list to reach.
        .safeAreaInset(edge: .bottom, spacing: 0) {
            sidebarDestinations(showsCalendarAndContacts: showsCalendarAndContacts)
        }
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

    /// The **Outbox** row: every account's unsent messages, above the trees, and **only while
    /// there are some** (`docs/folder-pane.md`, rule 18).
    ///
    /// Not inside a tree, because it is not a folder on anybody's server, and not per account,
    /// because "did that go?" is not a question about a particular mailbox. It carries the
    /// count as its badge, which is the one place in this pane a number is drawn for something
    /// other than unread mail; at zero the row is absent rather than saying nothing.
    @ViewBuilder private var outboxRow: some View {
        if !model.outbox.isEmpty {
            sidebarRow(
                title: L10n.folder_outbox(),
                icon: "tray.and.arrow.up",
                selected: showingMail && model.showingOutbox,
                unread: UInt32(model.outbox.count)
            ) { model.showOutbox() }
        }
    }

    /// The **All Accounts** group: every account's mail together, as one more tree in the pane,
    /// above the accounts it sums.
    ///
    /// **The row navigates nowhere**, so the whole row is the disclosure rather than a chevron
    /// beside a destination; its Inbox child is what opens the unified list. The account rows
    /// below work the other way round for the opposite reason: they *are* a destination, so
    /// their chevron has to be a control of its own (`docs/folder-pane.md`).
    ///
    /// The badge is on the child, not here, for the reason an account row carries none: a roll-up
    /// would sit directly above an identical number on the row beneath it.
    @ViewBuilder private var allAccountsGroup: some View {
        sidebarRow(
            title: L10n.sidebar_all_accounts(),
            icon: model.unifiedExpanded ? "chevron.down" : "chevron.right",
            selected: false
        ) { model.setUnifiedExpanded(!model.unifiedExpanded) }
            // The glyph is a state, not a name, so the row is spoken as "All Accounts" and the
            // hint is what says which way activating it goes.
            .accessibilityHint(
                model.unifiedExpanded
                    ? L10n.a11y_collapse_account()
                    : L10n.a11y_expand_account()
            )
        if model.unifiedExpanded {
            HStack(spacing: 0) {
                disclosureSlot
                sidebarRow(
                    title: L10n.folder_inbox(),
                    icon: folderIcon(.inbox),
                    selected: showingMail && model.selectedAccount == nil,
                    unread: model.unifiedUnread
                ) { selectAccount(nil) }
            }
            .padding(.leading, indentWidth)
        }
    }

    /// The control that opens or shuts the folders inside `folder`, or the blank of the same
    /// width where it holds none.
    ///
    /// Its own control, never the row: opening a folder to see what is in it is not opening
    /// its mail, the same separation the account rows above have (`docs/folder-pane.md`). The
    /// blank is reserved either way, so the names line up down a branch instead of stepping
    /// left wherever a folder happens to hold nothing.
    @ViewBuilder
    func folderDisclosure(in account: String, folder: FolderRow) -> some View {
        if folder.hasChildren {
            Button {
                model.setFolderExpanded(account, folder.key, !folder.expanded)
            } label: {
                Image(systemName: folder.expanded ? "chevron.down" : "chevron.right")
                    .font(.caption)
                    .frame(width: chevronTargetWidth, height: 28)
                    .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .accessibilityLabel(
                folder.expanded ? L10n.a11y_collapse_folder() : L10n.a11y_expand_folder()
            )
        } else {
            disclosureSlot
        }
    }

    /// The space a disclosure control would take, for a row that has none to draw.
    private var disclosureSlot: some View {
        Color.clear.frame(width: chevronTargetWidth, height: 28)
    }

    @ViewBuilder
    func sidebarRow(
        title: String,
        icon: String,
        selected: Bool,
        unread: UInt32 = 0,
        action: @escaping () -> Void
    ) -> some View {
        Button(action: action) {
            sidebarRowLabel(title: title, icon: icon, unread: unread)
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

    /// What a row says: its name and, at the trailing edge, its unread count.
    ///
    /// Apart from the button and its background, which differ between a row **in** the list and
    /// one of the destinations pinned under it: `listRowBackground` reaches only a row inside a
    /// `List`, so the pinned rows draw their own.
    @ViewBuilder
    func sidebarRowLabel(title: String, icon: String, unread: UInt32) -> some View {
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
        // The whole row is the target, not just the words on it. A `Spacer` is not
        // hit-testable, so the gap the count sits beside swallowed every click landing in
        // the middle of a row, the wider the pane, the more of the row was dead.
        .contentShape(Rectangle())
    }
}
