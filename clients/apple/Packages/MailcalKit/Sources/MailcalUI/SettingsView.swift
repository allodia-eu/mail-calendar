// The unified Settings screen (macOS): a categorised, Outlook-style preferences window with a
// source-list sidebar (General / Calendar / Reading / Composing / Privacy / Accounts / Advanced /
// Diagnostics) beside a detail panel per category. The taxonomy and each category's controls are
// shared (SettingsCategory / SettingsCategoryDetail) so this desktop chrome, the iPad two-pane and
// the iPhone hub (SettingsHubView) can't drift. Presented as a sheet from the mail toolbar (⌘,).
// Kept in its own file so each stays under 500 lines.

import MailcalBindings
import SwiftUI

// macOS-only: this window's `NavigationSplitView` + single-selection `List(_:selection:rowContent:)`
// use SwiftUI initializers we drive differently on iOS (SettingsHubView adapts the same taxonomy to
// a two-pane split on iPad and a hub-and-spoke on iPhone). Excluding the whole file from the iOS
// build keeps its macOS-shaped chrome from breaking the shared MailcalUI module compile there.
#if os(macOS)

struct SettingsView: View {
    var model: MailboxModel
    let close: () -> Void
    /// Removes an account by id, handed straight to `SettingsCategoryDetail`, see its own
    /// note for why removal is the shell's job rather than a direct model call.
    let removeAccount: (String) -> Void

    @State private var selection: SettingsCategory

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
        VStack(spacing: 0) {
            HStack {
                Text(L10n.settings_title()).font(.title2).bold()
                Spacer()
                Button(L10n.action_done(), action: close).keyboardShortcut(.defaultAction)
            }
            .padding()
            Divider()
            NavigationSplitView {
                List(SettingsCategory.displayed, selection: $selection) { category in
                    Label(category.title, systemImage: category.icon).tag(category)
                }
                .navigationSplitViewColumnWidth(200)
            } detail: {
                ScrollView {
                    SettingsCategoryDetail(model: model, category: selection, close: close, removeAccount: removeAccount)
                        .padding(20)
                        .frame(maxWidth: .infinity, alignment: .leading)
                }
            }
        }
        .frame(width: 720, height: 520)
    }
}

#endif
