// The own endpoint's form as drawn (AiEndpointSettings.swift): nothing while the record is absent,
// Remove only when offered, and a running Save holding the fields. Hosted in a window of its own
// with a model holding the record, so no core is needed; a gap named is the core's answer, and so
// `AiEndpointSettingsTests` in the UI suite.

#if os(iOS) && DEBUG
import MailcalBindings
import SwiftUI
import Testing

@testable import MailcalUI

@Suite @MainActor struct AiEndpointScreenTests {
    private func screen(_ form: AiEndpointForm?) -> HostedScreen {
        let model = MailboxModel()
        model.aiEndpoint = form
        // Tall enough that the whole form is on screen, Save included.
        return HostedScreen(
            AiEndpointSettings(model: model), size: CGSize(width: 393, height: 1400))
    }

    private func form(
        save: ControlState = .free(), remove: ControlState? = nil
    ) -> AiEndpointForm {
        AiEndpointForm(
            address: "http://localhost:11434/v1", model: "mistral", declared: .euNative,
            keyKept: false, save: save, remove: remove, outcome: nil)
    }

    @Test func nothingIsDrawnWhileTheFeatureIsNotOffered() {
        let shown = screen(nil)
        #expect(shown.elements("Save").isEmpty, "\(shown.dump())")
    }

    @Test func saveIsFreeWithNothingMissing() {
        let shown = screen(form())
        #expect(shown.elements("Save").first?.enabled == true, "\(shown.dump())")
    }

    @Test func removeIsDrawnOnlyWhenTheRecordOffersIt() {
        #expect(screen(form()).elements("Remove").isEmpty)
        let stored = screen(form(remove: .free()))
        #expect(stored.elements("Remove").first?.enabled == true, "\(stored.dump())")
    }

    /// The spinner beside Save is a SwiftUI `ProgressView`, which has no element in a test host;
    /// it is `ControlButton`'s, drawn for every running control.
    @Test func aRunningSaveHoldsItsFields() {
        let shown = screen(form(save: ControlState(enabled: false, running: true)))
        #expect(shown.elements("Save").first?.enabled == false, "\(shown.dump())")
        let address = shown.elements(role: "AXTextField") { _ in true }
        #expect(!address.isEmpty && address.allSatisfy { !$0.enabled }, "\(shown.dump())")
    }
}
#endif
