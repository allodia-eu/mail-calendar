// What the own endpoint's form does with the core's record that is not drawing (docs/ai.md rules 6
// and 7): the fields as the person is leaving them, the key a save sends, and what the form does
// with each outcome, once. Plain types, so the unit suite reads them without a screen.

import MailcalBindings

/// The form's fields as the person is leaving them, seeded from the record.
struct AiEndpointFields: Equatable {
    var address: String
    var model: String
    var declared: JurisdictionClass?
    /// The key as typed. Never seeded: a kept key is not drawn back.
    var key = ""

    init(_ form: AiEndpointForm) {
        address = form.address
        model = form.model
        declared = form.declared
    }

    /// The key a save sends: what was typed replaces the kept one, and an empty field leaves the
    /// kept one as it is.
    var keyToSend: String? {
        key.isEmpty ? nil : key
    }

    /// The intent that saves these fields as typed; the core trims each.
    var save: AiEndpointIntent {
        .save(address: address, model: model, declared: declared, key: keyToSend)
    }
}

/// What the form does with an outcome it has not acted on yet.
enum AiEndpointVerdict: Equatable {
    /// Saved, removed, or made and not written: the fields are the record's again and the key
    /// field empties. A change not written is the failed-save notice's to say.
    case adopt
    /// Not saved: the fields keep the edit, the key as typed included, and `problem` is said on
    /// the form when the gaps beside the fields do not already say it.
    case keep(problem: String?)
}

/// The form's hold on the record's outcomes: the newest it has acted on.
struct AiEndpointSession {
    private var seen: UInt64

    /// A form opening now, which an outcome already on the record does not concern.
    init(form: AiEndpointForm?) {
        seen = form?.outcome?.serial ?? 0
    }

    /// What to do with `outcome`, once: `nil` for one this form has seen.
    mutating func take(_ outcome: AiEndpointOutcome?) -> AiEndpointVerdict? {
        guard let outcome, outcome.serial > seen else { return nil }
        seen = outcome.serial
        switch outcome.result {
        case .saved, .removed, .notWritten: return .adopt
        case .refused: return .keep(problem: nil)
        case .keyNotStored: return .keep(problem: L10n.ai_endpoint_key_not_stored())
        }
    }
}
