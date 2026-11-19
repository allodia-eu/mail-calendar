// The writing assistant on the model (docs/ai.md): the own endpoint's form under Settings →
// Advanced. `model.aiEndpoint` is the core's `ai_endpoint_form`, read again on every
// `Surface::AiEndpoint` and `Surface::Settings` signal and `nil` while the feature is not offered.

import MailcalBindings

extension MailboxModel {
    /// What a save of these fields would refuse, each field's at once: the same function the save
    /// acts on. Read from memory, so the form asks it as the person types.
    func aiEndpointGap(_ fields: AiEndpointFields) -> AiEndpointGaps {
        app?.aiEndpointGap(address: fields.address, model: fields.model, declared: fields.declared)
            ?? .none
    }

    /// Saves the fields as typed. Returns at once; what it came to is the form's `outcome`.
    func saveAiEndpoint(_ fields: AiEndpointFields) {
        app?.dispatch(intent: .ai(intent: .endpoint(intent: fields.save)))
    }

    /// Removes the endpoint and its key. Returns at once.
    func removeAiEndpoint() {
        app?.dispatch(intent: .ai(intent: .endpoint(intent: .remove)))
    }
}

extension AiEndpointGaps {
    /// No gap: what the form draws before the core has opened.
    static var none: AiEndpointGaps {
        AiEndpointGaps(address: nil, modelMissing: false, declaration: nil)
    }

    /// Whether a save of these fields would be refused.
    var any: Bool {
        address != nil || modelMissing || declaration != nil
    }
}
