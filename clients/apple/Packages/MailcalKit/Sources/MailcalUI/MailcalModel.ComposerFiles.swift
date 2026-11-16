// A composer's files, as the core holds them (`docs/composer-layout.md`, rule 3): the composer reads
// the list here and changes it only through the core, which answers each change with the list as it
// now stands.

import Foundation
import MailcalBindings

extension MailboxModel {
    /// The files `composition`'s composer shows. Empty when there is no core, or no such
    /// composition.
    func composerFiles(_ composition: String) -> ComposerFiles {
        (try? app?.composerFiles(composition: composition)) ?? Self.noComposerFiles
    }

    /// Attaches files that are ready now, after what the composition already holds: a share's,
    /// as the composer opens.
    ///
    /// Off the main thread: the core reads each file's size and first bytes, and a file on a
    /// network volume or in iCloud can take as long as the network does (`docs/waiting.md`).
    func addComposerFiles(_ composition: String, _ files: [ComposerFileAttachment]) async -> ComposerFiles {
        guard !files.isEmpty, let app else { return composerFiles(composition) }
        let added = await Task.detached(priority: .userInitiated) {
            try? app.addComposerFiles(composition: composition, files: files)
        }.value
        return added ?? composerFiles(composition)
    }

    /// Starts adding files the person chose that are still being copied or read: the composer's
    /// Attach runs until `finishAddingComposerFiles` ends it. Only the core's lock, so on the main
    /// thread.
    func beginAddingComposerFiles(_ composition: String) -> ComposerFiles {
        (try? app?.beginAddingComposerFiles(composition: composition)) ?? composerFiles(composition)
    }

    /// Ends one addition with the files that arrived, none when nothing did, off the main thread
    /// as `addComposerFiles` is. Answers with the list read afterwards rather than with the call's
    /// own answer: two additions can end in either order, and the one drawn last must be whole.
    func finishAddingComposerFiles(_ composition: String, _ files: [ComposerFileAttachment]) async -> ComposerFiles {
        if let app {
            _ = await Task.detached(priority: .userInitiated) {
                try? app.finishAddingComposerFiles(composition: composition, files: files)
            }.value
        }
        return composerFiles(composition)
    }

    /// Takes one file, by its tile's id, off the composition.
    func removeComposerFile(_ composition: String, _ file: String) -> ComposerFiles {
        (try? app?.removeComposerFile(composition: composition, file: file))
            ?? composerFiles(composition)
    }

    static let noComposerFiles = ComposerFiles(tiles: [], changed: false, attach: .free())
}
