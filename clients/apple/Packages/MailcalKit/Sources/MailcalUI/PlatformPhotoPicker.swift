// Choosing pictures and films from the photo library on iPhone and iPad.
//
// The document picker that attaches a file does not reach the photo library: it lists Files'
// locations, and a photograph is in none of them. So Attach and a signature's Add image offer the
// library beside it, as the system's own picker, which runs in a process of its own and hands over
// only what was chosen. No library permission is asked for, and none is needed.

#if os(iOS)
import Foundation
import MailcalBindings
import MailcalShareBox
import PhotosUI
import SwiftUI
import UIKit

/// Where a file can come from on iPhone and iPad, as one menu: the photo library, then Files.
/// Attach and a signature's Add image both draw it. `control` is the core's state for the work it
/// starts, as Attach's while chosen files are still being copied (`docs/waiting.md`).
struct PhotosOrFilesMenu<MenuLabel: View>: View {
    var control: ControlState = .free()
    let photos: () -> Void
    let files: () -> Void
    @ViewBuilder let label: () -> MenuLabel

    var body: some View {
        ControlMenu(control: control) {
            Button(L10n.action_attach_photo_library(), systemImage: "photo.on.rectangle", action: photos)
            Button(L10n.action_attach_from_files(), systemImage: "folder", action: files)
        } label: {
            label()
        }
    }
}

/// The system photo picker, answering with copies of what was chosen. Retains itself while it is on
/// screen, as `PlatformFilePicker` does.
@MainActor
final class PlatformPhotoPicker: NSObject, PHPickerViewControllerDelegate {
    private static var active: PlatformPhotoPicker?
    private let onCopying: () -> Void
    private let onPick: (PhotoPick) -> Void
    private init(onCopying: @escaping () -> Void, onPick: @escaping (PhotoPick) -> Void) {
        self.onCopying = onCopying
        self.onPick = onPick
    }

    /// Presents the picker. `limit` is how many may be chosen, 0 for any number. `onCopying` is
    /// told when the picker has closed with something chosen and the copies begin, which is
    /// before `onPick` by as long as the copies take.
    static func present(
        filter: PHPickerFilter,
        limit: Int,
        onCopying: @escaping () -> Void = {},
        onPick: @escaping (PhotoPick) -> Void
    ) {
        guard let host = topPresentedViewController() else { return }
        let picker = PlatformPhotoPicker(onCopying: onCopying, onPick: onPick)
        active = picker
        let controller = PHPickerViewController(configuration: configuration(filter: filter, limit: limit))
        controller.delegate = picker
        host.present(controller, animated: true)
    }

    /// The picker's configuration, separate so a test can read it.
    ///
    /// **Compatible**, not current: a photograph the camera took as HEIC is handed over as a JPEG,
    /// as Apple's own Mail attaches it. A recipient on another platform may have nothing that opens
    /// HEIC, and the composer's tile previews only what every client decodes.
    static func configuration(filter: PHPickerFilter, limit: Int) -> PHPickerConfiguration {
        var configuration = PHPickerConfiguration()
        configuration.filter = filter
        configuration.selectionLimit = limit
        configuration.selection = .ordered
        configuration.preferredAssetRepresentationMode = .compatible
        return configuration
    }

    func picker(_ picker: PHPickerViewController, didFinishPicking results: [PHPickerResult]) {
        // Unlike the document picker, this one stays up until it is told to go.
        picker.dismiss(animated: true)
        let providers = results.map(\.itemProvider)
        let onPick = onPick
        Self.active = nil
        guard !providers.isEmpty else { return }
        onCopying()
        Task { @MainActor in
            onPick(await PhotoPick.copy(providers))
        }
    }
}

/// What came back from the photo picker: the copies made, and the names of what could not be
/// copied, so a picture that is not on this device and could not be downloaded is named rather than
/// silently left out.
struct PhotoPick {
    var files: [URL] = []
    var failed: [String] = []

    /// Copies each chosen item into a directory of its own under the app's temporary directory,
    /// where the document picker leaves its copies too: the core reads the file again at Send.
    ///
    /// Staged the way a share is (`ShareBox`), so a pick sweeps the copies older than a share
    /// would be kept: a film is hundreds of megabytes, and nothing else ever removes them.
    ///
    /// Every item's load starts before the first is awaited, so several films are transcoded side
    /// by side rather than one after another; the results keep the order they were chosen in.
    @MainActor
    static func copy(_ providers: [NSItemProvider]) async -> PhotoPick {
        let box = ShareBox(container: FileManager.default.temporaryDirectory
            .appendingPathComponent("picked-photos", isDirectory: true))
        box.sweep()
        var pick = PhotoPick()
        guard let directory = try? box.stagingDirectory(for: UUID().uuidString) else {
            pick.failed = providers.map { $0.suggestedName ?? "" }
            return pick
        }
        // A directory per item: two copied side by side under one name would both find it free.
        let copies = providers.enumerated().map { index, provider in
            Task {
                await ProvidedFile.copy(
                    provider, into: directory.appendingPathComponent("\(index)", isDirectory: true))
            }
        }
        for (provider, copy) in zip(providers, copies) {
            if let url = await copy.value {
                pick.files.append(url)
            } else {
                pick.failed.append(provider.suggestedName ?? "")
            }
        }
        return pick
    }
}
#endif
