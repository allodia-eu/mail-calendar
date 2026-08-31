// The unified Settings screen for touch (iOS/iPadOS), the same taxonomy as the macOS sidebar and
// the Windows source-list (docs/settings.md), adapted to each form factor: iPad shows a two-pane
// split (category list beside the detail, like macOS), iPhone shows a hub-and-spoke (a list of
// category rows, each pushing its own screen). Both render the shared SettingsCategoryDetail per
// category, so the three chromes can't drift. Presented as a sheet from the mail toolbar.

#if os(iOS)

import MailcalBindings
import SwiftUI

struct SettingsHubView: View {
    var model: MailboxModel
    let close: () -> Void
    /// Removes an account by id, handed straight to `SettingsCategoryDetail`, see its own
    /// note for why removal is the shell's job rather than a direct model call.
    let removeAccount: (String) -> Void

    @Environment(\.horizontalSizeClass) private var hSize
    // iPad's two-pane selection. Optional because a NavigationSplitView sidebar drives the detail
    // through an optional binding; it starts on the category the opener named, so the detail is
    // never blank.
    @State private var selection: SettingsCategory?

    /// Opens on `openOn`, every caller names one, because a caller that is pointing at a
    /// specific setting must land on it. An explicit `init` because `@State`'s default cannot
    /// read another property.
    init(
        model: MailboxModel,
        close: @escaping () -> Void,
        removeAccount: @escaping (String) -> Void,
        openOn: SettingsCategory
    ) {
        self.model = model
        self.close = close
        self.removeAccount = removeAccount
        _selection = State(initialValue: openOn)
    }

    var body: some View {
        if hSize == .compact { iPhoneHub } else { iPadSplit }
    }

    /// iPhone: a hub of category rows, each pushing its detail. Back (arrow and system) steps
    /// detail → hub; Done closes the sheet.
    private var iPhoneHub: some View {
        NavigationStack {
            List(SettingsCategory.displayed) { category in
                NavigationLink {
                    // Scrolled, like the iPad detail beside it. A category's detail is as tall as
                    // its content, Accounts grows a card per account, each with its own pickers:
                    // and a plain view in a NavigationStack does not scroll, so everything past the
                    // first screenful was unreachable rather than merely below the fold. It looks
                    // like a screen that simply ends.
                    ScrollView {
                        SettingsCategoryDetail(model: model, category: category, close: close, removeAccount: removeAccount)
                            .padding(20)
                            .frame(maxWidth: .infinity, alignment: .leading)
                    }
                    .navigationTitle(category.title)
                    .navigationBarTitleDisplayMode(.inline)
                } label: {
                    hubRow(category)
                }
            }
            .navigationTitle(L10n.settings_title())
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .topBarTrailing) {
                    Button(L10n.action_done(), action: close)
                }
            }
        }
    }

    /// iPad: category list beside the detail, mirroring macOS. Wrapping the detail in its own
    /// NavigationStack keeps the section title visible in the detail column.
    private var iPadSplit: some View {
        NavigationSplitView {
            List(SettingsCategory.displayed, selection: $selection) { category in
                Label(category.title, systemImage: category.icon).tag(category)
            }
            .navigationTitle(L10n.settings_title())
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .topBarTrailing) {
                    Button(L10n.action_done(), action: close)
                }
            }
        } detail: {
            NavigationStack {
                ScrollView {
                    SettingsCategoryDetail(model: model, category: selection ?? .general, close: close, removeAccount: removeAccount)
                        .padding(20)
                        .frame(maxWidth: .infinity, alignment: .leading)
                }
                .navigationTitle((selection ?? .general).title)
                .navigationBarTitleDisplayMode(.inline)
            }
        }
    }

    /// One iPhone hub row: the category's icon, its name, and a one-line summary of what's inside:
    /// so a first-time user can find a setting without opening every category. NavigationLink adds
    /// the disclosure chevron.
    @ViewBuilder
    private func hubRow(_ category: SettingsCategory) -> some View {
        HStack(spacing: 14) {
            Image(systemName: category.icon)
                .font(.title3)
                .foregroundStyle(.tint)
                .frame(width: 28)
            VStack(alignment: .leading, spacing: 2) {
                Text(category.title).font(.body)
                Text(category.summary)
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    .fixedSize(horizontal: false, vertical: true)
            }
        }
        .padding(.vertical, 4)
    }
}

#endif
