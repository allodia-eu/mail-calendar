// Which way a Gmail account's mail syncs in this launch, on its page (docs/settings.md row 9):
// Optimised, Fallback, or Checking until the first read has settled it. The status never names a
// protocol; the info button says what it means and links to the help page.

import MailcalBindings
import SwiftUI

struct AccountSyncTechnologySection: View {
    let technology: MailSyncTechnology

    @State private var explaining = false
    @Environment(\.openURL) private var openURL

    var body: some View {
        GroupBox {
            HStack(spacing: 8) {
                Text(L10n.settings_account_sync_technology_heading()).font(.headline)
                Spacer()
                status
                Button {
                    explaining = true
                } label: {
                    Image(systemName: "info.circle")
                        #if os(iOS)
                        .frame(minWidth: 44, minHeight: 44)
                        .contentShape(Rectangle())
                        #endif
                }
                .buttonStyle(.borderless)
                .accessibilityLabel(L10n.settings_account_sync_technology_about())
                .popover(isPresented: $explaining, arrowEdge: .bottom) {
                    explanation.presentationCompactAdaptation(.popover)
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(6)
        }
    }

    private var status: some View {
        HStack(spacing: 6) {
            Circle()
                .fill(AccountSyncTechnologyLabels.color(technology))
                .frame(width: 8, height: 8)
                .accessibilityHidden(true)
            Text(AccountSyncTechnologyLabels.name(technology))
        }
        .accessibilityElement(children: .combine)
    }

    private var explanation: some View {
        VStack(alignment: .leading, spacing: 10) {
            Text(AccountSyncTechnologyLabels.detail(technology))
                .fixedSize(horizontal: false, vertical: true)
            Button(L10n.settings_account_sync_technology_learn_more()) {
                if let url = URL(string: helpPageUrl(page: .optimiseGmailSync)) { openURL(url) }
                explaining = false
            }
            .modifier(LinkButtonStyle())
        }
        .padding(14)
        .frame(idealWidth: 280, maxWidth: 320, alignment: .leading)
    }
}

/// What each sync technology is called and how it is drawn, apart from the view so it can be
/// tested.
enum AccountSyncTechnologyLabels {
    static func name(_ technology: MailSyncTechnology) -> String {
        switch technology {
        case .checking: L10n.settings_account_sync_technology_checking()
        case .optimised: L10n.settings_account_sync_technology_optimised()
        case .fallback: L10n.settings_account_sync_technology_fallback()
        }
    }

    static func detail(_ technology: MailSyncTechnology) -> String {
        switch technology {
        case .checking: L10n.settings_account_sync_technology_checking_detail()
        case .optimised: L10n.settings_account_sync_technology_optimised_detail()
        case .fallback: L10n.settings_account_sync_technology_fallback_detail()
        }
    }

    static func color(_ technology: MailSyncTechnology) -> Color {
        switch technology {
        case .checking: .secondary
        case .optimised: .green
        case .fallback: .orange
        }
    }
}
