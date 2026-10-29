// Settings → Accounts: every account, mail or not, from the core's one snapshot, each opening a
// page of its own (docs/accounts.md rule 11, docs/settings.md row 9). On macOS the page replaces
// the list inside the detail panel, with a way back; on iOS and iPadOS it is pushed onto the
// navigation stack the category already sits in.

import MailcalBindings
import SwiftUI

struct AccountsSettingsView: View {
    var model: MailboxModel
    let close: () -> Void
    /// The shell's removal, which also clears the scene's selection when it named this account.
    /// A failure to erase the stored sign-in arrives as `model.removalFailure`.
    let removeAccount: (String) -> Void

    var body: some View {
        content.modifier(RemovalFailureAlert(model: model))
    }

    @ViewBuilder
    private var content: some View {
        #if os(macOS)
        if let id = model.accountPage, let entry = model.accountEntry(id) {
            AccountPageView(model: model, entry: entry, remove: remove)
                .id(entry.id)
        } else {
            list
        }
        #else
        list.navigationDestination(item: pageBinding) { id in
            AccountPageScreen(model: model, id: id, remove: remove)
        }
        #endif
    }

    private var pageBinding: Binding<String?> {
        Binding(get: { model.accountPage }, set: { model.accountPage = $0 })
    }

    private func remove(_ id: String) {
        model.accountPage = nil
        removeAccount(id)
    }

    private var list: some View {
        VStack(alignment: .leading, spacing: 20) {
            // The only place a later account is added (docs/settings.md row 9). It closes
            // Settings the way an offer below does, for the reason given there.
            Button(L10n.action_add_account()) {
                model.setupError = nil
                model.addAccountWhenSettingsCloses = true
                close()
            }
            .buttonStyle(.borderedProminent)
            .accessibilityIdentifier("settings-add-account")
            // What the person's other devices have to say, above their own accounts: an offer
            // becomes one of them.
            // Settings is a modal, and the setup sheet cannot present over it, so this asks for
            // it and closes, and `ContentView` opens it once this is off screen.
            AllodiaSyncSettings(model: model) { offer in
                model.setupStartEmail = offer.email
                model.setupStartOffer = offer
                model.addAccountWhenSettingsCloses = true
                close()
            }
            if model.accountsSnapshot.accounts.isEmpty {
                Text(L10n.settings_accounts_empty())
                    .foregroundStyle(.secondary)
            } else {
                GroupBox {
                    VStack(alignment: .leading, spacing: 0) {
                        ForEach(
                            Array(model.accountsSnapshot.accounts.enumerated()), id: \.element.id
                        ) { index, entry in
                            if index > 0 { Divider() }
                            row(entry)
                        }
                    }
                }
            }
        }
    }

    /// One account: its address, then its kind, uses and links, and an arrow to its page.
    private func row(_ entry: AccountEntry) -> some View {
        Button {
            model.accountPage = entry.id
        } label: {
            HStack(spacing: 10) {
                VStack(alignment: .leading, spacing: 2) {
                    Text(entry.address).font(.body)
                    ForEach(AccountSettingsLabels.summary(entry), id: \.self) { line in
                        Text(line)
                            .font(.caption)
                            .foregroundStyle(.secondary)
                            .fixedSize(horizontal: false, vertical: true)
                    }
                }
                Spacer(minLength: 0)
                Image(systemName: "chevron.right")
                    .font(.caption.weight(.semibold))
                    .foregroundStyle(.tertiary)
                    .accessibilityHidden(true)
            }
            .padding(.vertical, 8)
            .padding(.horizontal, 4)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .accessibilityIdentifier("settings-account-\(entry.address)")
    }
}

#if os(iOS)
/// The pushed page. Its own view, so the snapshot is read in a body SwiftUI observes: a
/// `navigationDestination` closure is not one, and a page built there never redraws.
private struct AccountPageScreen: View {
    var model: MailboxModel
    let id: String
    let remove: (String) -> Void

    var body: some View {
        if let entry = model.accountEntry(id) {
            ScrollView {
                AccountPageView(model: model, entry: entry, remove: remove)
                    .id(entry.id)
                    .padding(20)
                    .frame(maxWidth: .infinity, alignment: .leading)
            }
            .navigationTitle(entry.address)
            .navigationBarTitleDisplayMode(.inline)
        }
    }
}
#endif

/// Says that a removed account's stored sign-in could not be erased, so it would come back at the
/// next launch. On the account list, and on the shell for a removal from the sidebar or of the last
/// account, which takes Settings away with it.
struct RemovalFailureAlert: ViewModifier {
    var model: MailboxModel
    /// Whether this is where it is said now.
    var showing = true

    func body(content: Content) -> some View {
        content.alert(
            model.removalFailure ?? "",
            isPresented: Binding(
                get: { showing && model.removalFailure != nil },
                set: { if !$0 { model.removalFailure = nil } }
            )
        ) {
            Button(L10n.action_close(), role: .cancel) {}
        }
    }
}

/// A labelled settings section: a heading, a line saying what it is for, and its controls, in the
/// card every category draws.
struct SettingsGroup<Content: View>: View {
    let heading: String
    let description: String
    @ViewBuilder var content: () -> Content

    var body: some View {
        GroupBox {
            VStack(alignment: .leading, spacing: 8) {
                Text(heading).font(.headline)
                Text(description).font(.callout).foregroundStyle(.secondary)
                    .fixedSize(horizontal: false, vertical: true)
                content().padding(.top, 2)
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(6)
        }
    }
}

/// How an account is shared with the person's other devices: one control in three positions.
///
/// A segmented picker rather than a switch and a button: the two questions underneath, is this
/// account on my other devices, and does this device exchange changes about it, are not independent
/// in any way somebody can act on, and splitting them produced a screen where turning the switch
/// off changed nothing the person could see.
///
/// One subtext, the selected position's: three at once is a paragraph nobody reads, and the one
/// that matters is the one in force.
struct AccountSyncModePicker: View {
    var model: MailboxModel
    let accountId: String
    let mode: AllodiaAccountSyncMode

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            Text(L10n.settings_account_sync_heading()).font(.subheadline).bold()
            Picker(
                L10n.settings_account_sync_heading(),
                selection: Binding(
                    get: { mode },
                    set: { picked in
                        guard picked != mode else { return }
                        Task { await model.setAllodiaAccountSyncMode(accountId, picked) }
                    }
                )
            ) {
                Text(L10n.settings_account_sync_on()).tag(AllodiaAccountSyncMode.on)
                Text(L10n.settings_account_sync_paused()).tag(AllodiaAccountSyncMode.paused)
                Text(L10n.settings_account_sync_off()).tag(AllodiaAccountSyncMode.off)
            }
            .pickerStyle(.segmented)
            .labelsHidden()
            Text(hint)
                .font(.caption)
                .foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)
        }
    }

    private var hint: String {
        switch mode {
        case .on: L10n.settings_account_sync_on_hint()
        case .paused: L10n.settings_account_sync_paused_hint()
        case .off: L10n.settings_account_sync_off_hint()
        }
    }
}
