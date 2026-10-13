//! Every string the shared editor's own chrome draws, in the bundle's key names.

use serde_json::Value;

use crate::l10n;

/// One map for the composer **and** the Settings signature editor: the bundle keeps no
/// translations of its own, and a host that sends a partial map leaves those controls in English
/// with nothing to say so (`cargo xtask check-composer-labels`).
///
/// Built from pairs rather than with `json!`, whose expansion recurses once per token and passes
/// the compiler's default recursion limit long before a map this size.
pub(super) fn editor_labels() -> Value {
    let labels = [
        ("placeholder", l10n::editor_placeholder()),
        ("bold", l10n::editor_bold()),
        ("italic", l10n::editor_italic()),
        ("underline", l10n::editor_underline()),
        ("fontSize", l10n::editor_font_size()),
        ("sizeNormal", l10n::editor_size_normal()),
        ("sizeSmall", l10n::editor_size_small()),
        ("sizeLarge", l10n::editor_size_large()),
        ("sizeHuge", l10n::editor_size_huge()),
        ("fontFamily", l10n::editor_font_family()),
        ("fontDefault", l10n::editor_font_default()),
        ("blockStyle", l10n::editor_block_style()),
        ("blockNormal", l10n::editor_block_normal()),
        ("bulletedList", l10n::editor_bulleted_list()),
        ("numberedList", l10n::editor_numbered_list()),
        ("indent", l10n::editor_indent()),
        ("outdent", l10n::editor_outdent()),
        ("align", l10n::editor_align()),
        ("alignLeft", l10n::editor_align_left()),
        ("alignCentre", l10n::editor_align_centre()),
        ("alignRight", l10n::editor_align_right()),
        ("alignJustify", l10n::editor_align_justify()),
        ("horizontalRule", l10n::editor_horizontal_rule()),
        ("textColour", l10n::editor_text_colour()),
        ("colourAutomatic", l10n::editor_colour_automatic()),
        ("highlight", l10n::editor_highlight()),
        ("highlightNone", l10n::editor_highlight_none()),
        ("table", l10n::editor_table()),
        ("insertTable", l10n::editor_insert_table()),
        ("insertRowAbove", l10n::editor_insert_row_above()),
        ("insertRowBelow", l10n::editor_insert_row_below()),
        ("insertColumnLeft", l10n::editor_insert_column_left()),
        ("insertColumnRight", l10n::editor_insert_column_right()),
        ("deleteRow", l10n::editor_delete_row()),
        ("deleteColumn", l10n::editor_delete_column()),
        ("deleteTable", l10n::editor_delete_table()),
        ("link", l10n::editor_link()),
        ("linkText", l10n::editor_link_text()),
        ("linkAddress", l10n::editor_link_address()),
        ("linkApply", l10n::editor_link_apply()),
        ("linkRemove", l10n::editor_link_remove()),
    ];
    // A heading's name carries its level, so the catalog answers it with an owned string.
    let headings = [
        ("heading1", l10n::editor_heading("1")),
        ("heading2", l10n::editor_heading("2")),
        ("heading3", l10n::editor_heading("3")),
        ("heading4", l10n::editor_heading("4")),
        ("heading5", l10n::editor_heading("5")),
        ("heading6", l10n::editor_heading("6")),
    ];
    Value::Object(
        labels
            .into_iter()
            .map(|(key, text)| (key.to_owned(), Value::String(text.to_owned())))
            .chain(
                headings
                    .into_iter()
                    .map(|(key, text)| (key.to_owned(), Value::String(text))),
            )
            .collect(),
    )
}
