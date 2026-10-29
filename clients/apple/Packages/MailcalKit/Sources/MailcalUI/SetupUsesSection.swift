// What a found card offers the account for: mail, calendar and contacts, each a choice the core
// decided how to start (docs/account-autodetect.md rule 8), and the servers detection found for
// them. The rules are `UseChoices`'; this draws them.

import MailcalBindings
import SwiftUI

/// One card per use the core offered, in its order. `mailRows` is what the mail card shows while
/// mail is on: the servers detection found.
struct SetupUsesSection<MailRows: View>: View {
    @Binding var choices: UseChoices
    /// The field still to fill in, marked and focused by an action that cannot go on without it.
    @Binding var flagged: MissingServer
    @ViewBuilder var mailRows: () -> MailRows

    @FocusState private var focused: AccountCapability?

    var body: some View {
        ForEach(choices.offer.choices.map(\.capability), id: \.self) { capability in
            switch capability {
            case .mail: mailCard
            case .calendar: davCard(.calendar)
            case .contacts: davCard(.contacts)
            // Drawn inside the contacts card, beneath the toggle it depends on.
            case .colleagues: EmptyView()
            // Not offered: this client has no section to choose one (docs/shared-mailboxes.md §3).
            case .sharedMailboxes: EmptyView()
            }
        }
        .onChange(of: flagged) { _, missing in
            switch missing {
            case .calendar: focused = .calendar
            case .contacts: focused = .contacts
            case .none: break
            }
        }
    }

    private var mailCard: some View {
        SetupCard(title: L10n.setup_detect_section_email(), systemImage: "envelope") {
            toggle(.mail, L10n.setup_detect_mail_enable())
            // Following the toggle, so a use switched off leaves nothing behind claiming otherwise.
            if choices.isOn(.mail) { mailRows() }
        }
    }

    private func davCard(_ capability: AccountCapability) -> some View {
        let calendar = capability == .calendar
        let found = choices.offer.choice(capability)?.serverFound ?? false
        return SetupCard(
            title: calendar
                ? L10n.setup_detect_section_calendar() : L10n.setup_detect_section_contacts(),
            systemImage: calendar ? "calendar" : "person.2"
        ) {
            toggle(
                capability,
                calendar
                    ? (found ? L10n.setup_detect_calendar_enable() : L10n.setup_detect_calendar_add())
                    : (found ? L10n.setup_detect_contacts_enable() : L10n.setup_detect_contacts_add())
            )
            // Beneath the toggle: the endpoint detection found, to recognise; a box to type one
            // into when it found none; and nothing for a provider's sign-in, which covers the use
            // with no server to name.
            if choices.isOn(capability) {
                if !found {
                    serverField(capability, hint: calendar ? L10n.setup_hint_caldav() : L10n.setup_hint_carddav())
                } else if !choices.offer.found(capability).isEmpty {
                    Text(urlHost(choices.offer.found(capability)))
                        .font(.caption).foregroundStyle(.secondary)
                }
            }
            if !calendar, choices.offer.choice(.colleagues) != nil {
                // Colleagues come from the organisation's directory beside the person's own
                // contacts, so they are offered only while contacts are on.
                toggle(.colleagues, L10n.setup_detect_colleagues_enable())
                    .padding(.leading, 20)
            }
        }
    }

    private func toggle(_ capability: AccountCapability, _ label: String) -> some View {
        Toggle(
            label,
            isOn: Binding(
                get: { choices.isOn(capability) },
                set: { choices.set(capability, $0) }
            )
        )
        .disabled(!choices.canFlip(capability))
    }

    private func serverField(_ capability: AccountCapability, hint: String) -> some View {
        let marked = flagged == (capability == .calendar ? .calendar : .contacts)
        return TextField(
            hint,
            text: Binding(
                get: { choices.typed(capability) },
                set: {
                    choices.type(capability, $0)
                    flagged = .none
                }
            )
        )
        .setupField(.host)
        .focused($focused, equals: capability)
        // Marked for the eye; focus moving to it is what a screen reader announces.
        .overlay {
            if marked {
                RoundedRectangle(cornerRadius: 6).stroke(.red, lineWidth: 1.5)
            }
        }
    }
}
