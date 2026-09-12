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

extension ContentView {
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
                    // another account.
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
        if model.destination == .mail && model.unifiedExpanded {
            sidebarRow(
                title: L10n.folder_inbox(),
                icon: folderIcon(.inbox),
                selected: model.selectedAccount == nil,
                indent: true,
                unread: model.unifiedUnread
            ) { selectAccount(nil) }
        }
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
