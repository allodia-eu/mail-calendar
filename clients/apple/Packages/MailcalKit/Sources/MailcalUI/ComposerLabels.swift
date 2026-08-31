// The editor chrome's localised strings, in the shape `window.setComposerLabels` takes.
//
// The shared bundle ships English and every host is expected to send its own translations; a host
// that never calls the hook silently keeps English, which is what macOS and iOS did. The keys are
// the bundle's own (`clients/composer/src/labels.ts`) and the values come from the shared catalog,
// so the toolbar follows the app's language like every other surface.
//
// Both editor hosts use this, the composer and the Settings signature editor, because they load
// the same bundle and a localised toolbar in one of them only would be its own kind of odd.

import Foundation
import MailcalBindings

enum ComposerLabels {
    /// Every key the bundle's `Labels` declares, keyed exactly as it spells them.
    ///
    /// Both halves of a mismatch fail silently: a key the bundle does not know is dropped by
    /// `mergeLabels`, and one it knows but this omits keeps its English default. Nothing throws and
    /// nothing logs, so the set is pinned by `ComposerLabelsTests` and across the clients by
    /// `scripts/ci/check_composer_labels.py`.
    static func values() -> [String: String] {
        [
            "placeholder": L10n.editor_placeholder(),
            "bold": L10n.editor_bold(),
            "italic": L10n.editor_italic(),
            "underline": L10n.editor_underline(),
            "fontSize": L10n.editor_font_size(),
            "sizeNormal": L10n.editor_size_normal(),
            "sizeSmall": L10n.editor_size_small(),
            "sizeLarge": L10n.editor_size_large(),
            "sizeHuge": L10n.editor_size_huge(),
            "bulletedList": L10n.editor_bulleted_list(),
            "numberedList": L10n.editor_numbered_list(),
            "indent": L10n.editor_indent(),
            "outdent": L10n.editor_outdent(),
            "textColour": L10n.editor_text_colour(),
            "colourAutomatic": L10n.editor_colour_automatic(),
            "highlight": L10n.editor_highlight(),
            "highlightNone": L10n.editor_highlight_none(),
            "table": L10n.editor_table(),
            "insertTable": L10n.editor_insert_table(),
            "insertRowAbove": L10n.editor_insert_row_above(),
            "insertRowBelow": L10n.editor_insert_row_below(),
            "insertColumnLeft": L10n.editor_insert_column_left(),
            "insertColumnRight": L10n.editor_insert_column_right(),
            "deleteRow": L10n.editor_delete_row(),
            "deleteColumn": L10n.editor_delete_column(),
            "deleteTable": L10n.editor_delete_table(),
        ]
    }

    /// The `values()` as a JSON object literal, ready to interpolate into the hook call.
    ///
    /// `JSONSerialization` is what makes this safe to interpolate: a translation holding a quote or
    /// a backslash is escaped by it, not by us.
    static func json() -> String {
        guard let data = try? JSONSerialization.data(withJSONObject: values()),
              let literal = String(data: data, encoding: .utf8)
        else {
            // Unreachable for a dictionary of strings. English chrome is the right degradation:
            // it is what the bundle already shows.
            return "{}"
        }
        return literal
    }

    /// The call the hosts inject once the bundle has parsed.
    static func script() -> String {
        "window.setComposerLabels(\(json()))"
    }
}
