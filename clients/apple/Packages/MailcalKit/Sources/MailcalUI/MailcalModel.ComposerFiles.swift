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

    /// Attaches files picked, dropped or shared, after what the composition already holds.
    func addComposerFiles(_ composition: String, _ files: [ComposerFileAttachment]) -> ComposerFiles {
        guard !files.isEmpty else { return composerFiles(composition) }
        return (try? app?.addComposerFiles(composition: composition, files: files))
            ?? composerFiles(composition)
    }

    /// Takes one file, by its tile's id, off the composition.
    func removeComposerFile(_ composition: String, _ file: String) -> ComposerFiles {
        (try? app?.removeComposerFile(composition: composition, file: file))
            ?? composerFiles(composition)
    }

    static let noComposerFiles = ComposerFiles(tiles: [], changed: false)
}
