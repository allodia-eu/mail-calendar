// One person's detail: every value the core assembled for them, and which accounts supplied it.
//
// The section that earns this screen its keep is "Also in": for a person merged from several
// accounts it names them, which is the *explanation* of the "In 2 accounts" badge on the list row.
// Per-value provenance is shown for the same reason, an address the user only has at work should
// be visibly the work one (docs/contacts.md §1).

import MailcalBindings
import SwiftUI

struct ContactDetailView: View {
    let detail: ContactDetail
    /// Account id → the address the user knows that account by. The core's ids are internal
    /// (`alice@test.local@jmap:127.0.0.1:18080`); showing one is both ugly and a leak of how ids
    /// are built. Falls back to the id if an account has since been removed.
    let accountLabels: [String: String]

    /// How many accounts the whole person spans. With only one there is nothing to disambiguate,
    /// so the per-value account tags are suppressed rather than repeating the same account name
    /// down the screen.
    private var spansSeveralAccounts: Bool { detail.accounts.count > 1 }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                HStack(spacing: 12) {
                    AvatarView(avatar: detail.avatar, diameter: 56)
                    // Empty when every source card is nameless, the core has no locale, so the
                    // placeholder is ours to supply (docs/contacts.md §2).
                    Text(detail.displayName.isEmpty ? L10n.contacts_no_name() : detail.displayName)
                        .font(.title2)
                        .textSelection(.enabled)
                    Spacer(minLength: 0)
                }

                valueSection(L10n.contacts_section_emails(), detail.emails)
                valueSection(L10n.contacts_section_phones(), detail.phones)
                valueSection(L10n.contacts_section_organizations(), detail.organizations)
                valueSection(L10n.contacts_section_titles(), detail.titles)

                // Only shown for an actual merge: naming the single account an ordinary contact
                // came from is noise, and would make every contact look like a merge.
                if spansSeveralAccounts {
                    VStack(alignment: .leading, spacing: 4) {
                        sectionHeading(L10n.contacts_section_accounts())
                        ForEach(detail.accounts, id: \.self) { account in
                            Text(accountLabels[account] ?? account)
                        }
                    }
                }

                Divider()
                // Said in as many words, rather than left for the user to infer from the absence
                // of an edit button, or, worse, from a disabled one they press twice
                // (docs/contacts.md §3).
                Text(L10n.contacts_read_only())
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(20)
        }
    }

    /// One labelled group of values, each tagged with the accounts carrying it.
    @ViewBuilder
    private func valueSection(_ heading: String, _ values: [ContactValue]) -> some View {
        if !values.isEmpty {
            VStack(alignment: .leading, spacing: 6) {
                sectionHeading(heading)
                ForEach(values, id: \.value) { value in
                    // Stacked, not side by side. Laid out as a row, the provenance label, several
                    // full email addresses joined by commas, takes whatever width it wants and
                    // squeezes the value to nothing, which rendered an address one character per
                    // line on Android. A column cannot do that whatever either string's length.
                    VStack(alignment: .leading, spacing: 1) {
                        Text(value.value).textSelection(.enabled)
                        if spansSeveralAccounts {
                            Text(value.accounts.map { accountLabels[$0] ?? $0 }
                                .joined(separator: ", "))
                                .font(.caption)
                                .foregroundStyle(.secondary)
                        }
                    }
                }
            }
        }
    }

    private func sectionHeading(_ text: String) -> some View {
        Text(text)
            .font(.caption)
            .fontWeight(.semibold)
            .foregroundStyle(Color.accentColor)
    }
}

/// The contacts detail pane before a person is picked, the counterpart of the reading pane's
/// placeholder, so a two-column contacts layout is never a blank half-window.
struct ContactDetailPlaceholder: View {
    var body: some View {
        VStack(spacing: 8) {
            Image(systemName: "person.crop.circle")
                .font(.system(size: 40))
                .foregroundStyle(.secondary)
            Text(L10n.contacts_title()).foregroundStyle(.secondary)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }
}
