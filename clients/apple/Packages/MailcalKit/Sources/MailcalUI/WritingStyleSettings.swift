// Settings → Writing style (docs/ai.md, docs/settings.md row 7): the learned styles, then which one
// each account drafts in, as Signatures draws its library and its per-account slots. Present only
// while `writingStyles.route` says AI has somewhere to go (`SettingsCategory.displayed`).
//
// Learning starts here and asks for consent on its own sheet, at the point of use. A row opens the
// style, which is also where a new one lands once it has been learned.

import Foundation
import MailcalBindings
import SwiftUI

struct WritingStyleSettings: View {
    var model: MailboxModel

    /// Which sheet is open: the learning flow, or one style.
    private enum Sheet: Identifiable {
        case learn
        case style(String)

        var id: String {
            switch self {
            case .learn: "learn"
            case let .style(id): "style:\(id)"
            }
        }
    }

    @State private var sheet: Sheet?

    private var snapshot: WritingStyleSnapshot { model.writingStyles }

    var body: some View {
        VStack(alignment: .leading, spacing: 20) {
            group(L10n.settings_category_writing_style(), L10n.writing_style_intro()) {
                learnControl
                library
                if snapshot.route == .relay, let balance = snapshot.balance {
                    Text(writingStyleCreditsLine(
                        balance, now: Date(), zone: displayZone, locale: L10n.appLocale
                    ))
                    .font(.callout)
                    .foregroundStyle(.secondary)
                }
            }
            group(L10n.writing_style_accounts_heading(), nil) {
                AccountWritingStylePickers(model: model)
            }
        }
        .sheet(item: $sheet) { sheet in
            switch sheet {
            case .learn:
                LearnWritingStyleSheet(model: model) { learned in
                    self.sheet = learned.map(Sheet.style)
                }
            case let .style(id):
                WritingStyleRevealView(model: model, styleId: id) { self.sheet = nil }
            }
        }
    }

    /// Learn, or in its place why the gate would refuse: a button that can only fail is worse than
    /// the sentence saying what would have to change.
    @ViewBuilder
    private var learnControl: some View {
        if let refused = snapshot.refused {
            Text(writingStyleRefusalText(refused.mode))
                .font(.callout)
                .foregroundStyle(.orange)
                .fixedSize(horizontal: false, vertical: true)
        } else {
            Button {
                sheet = .learn
            } label: {
                Label(L10n.writing_style_learn(), systemImage: "text.quote")
            }
            .buttonStyle(.bordered)
            .controlSize(.small)
            .disabled(snapshot.learning != nil || snapshot.accounts.isEmpty)
        }
    }

    @ViewBuilder
    private var library: some View {
        if snapshot.styles.isEmpty {
            Text(L10n.writing_style_empty())
                .font(.callout)
                .foregroundStyle(.secondary)
        } else {
            ForEach(snapshot.styles, id: \.id) { style in
                row(style)
            }
        }
    }

    /// The whole row opens the style: nothing destructive sits beside it, forgetting is inside.
    private func row(_ style: WritingStyleRow) -> some View {
        Button {
            sheet = .style(style.id)
        } label: {
            HStack(spacing: 8) {
                Image(systemName: "text.quote").foregroundStyle(.secondary)
                VStack(alignment: .leading, spacing: 2) {
                    Text(style.name).lineLimit(1).truncationMode(.middle)
                    Text(L10n.writing_style_learned_from(
                        count: Int(style.messages),
                        date: writingStyleDate(style.learnedAt, zone: displayZone, locale: L10n.appLocale)
                    ))
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    if !style.languages.isEmpty {
                        Text(L10n.writing_style_languages(
                            languages: writingStyleLanguages(style.languages, locale: L10n.appLocale)
                        ))
                        .font(.caption)
                        .foregroundStyle(.secondary)
                    }
                }
                Spacer()
                Image(systemName: "chevron.right").foregroundStyle(.tertiary)
            }
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .padding(.vertical, 2)
    }

    private var displayZone: TimeZone {
        TimeZone(identifier: model.activeZone) ?? .current
    }

    /// A labelled section, matching the shape the sibling settings panels use.
    private func group(
        _ heading: String,
        _ description: String?,
        @ViewBuilder content: () -> some View
    ) -> some View {
        GroupBox {
            VStack(alignment: .leading, spacing: 8) {
                Text(heading).font(.headline)
                if let description {
                    Text(description).font(.callout).foregroundStyle(.secondary)
                        .fixedSize(horizontal: false, vertical: true)
                }
                VStack(alignment: .leading, spacing: 10) { content() }.padding(.top, 2)
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(6)
        }
    }
}

/// Which style each account drafts in, with "None".
struct AccountWritingStylePickers: View {
    var model: MailboxModel

    var body: some View {
        let snapshot = model.writingStyles
        if snapshot.accounts.isEmpty {
            Text(L10n.settings_accounts_empty())
                .font(.callout)
                .foregroundStyle(.secondary)
        } else {
            VStack(alignment: .leading, spacing: 10) {
                ForEach(snapshot.accounts, id: \.accountId) { account in
                    // The address beside the Picker rather than as its label: outside a Form a
                    // `.menu` Picker drops its label on iOS (AccountSignatureDefaults does the same).
                    HStack(spacing: 10) {
                        Text(account.email)
                            .font(.callout)
                            .lineLimit(1)
                            .truncationMode(.middle)
                            .frame(maxWidth: 220, alignment: .leading)
                        Picker(account.email, selection: binding(account)) {
                            Text(L10n.writing_style_none()).tag(String?.none)
                            ForEach(snapshot.styles, id: \.id) { style in
                                Text(style.name).tag(Optional(style.id))
                            }
                        }
                        .pickerStyle(.menu)
                        .labelsHidden()
                        .frame(maxWidth: 220, alignment: .leading)
                    }
                }
            }
        }
    }

    private func binding(_ account: AccountWritingStyleRow) -> Binding<String?> {
        Binding(
            get: { account.style },
            set: { model.setAccountWritingStyle(account.accountId, $0) }
        )
    }
}
