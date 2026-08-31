using System.Collections.Generic;
using System.Text.Json;

namespace Allodia.Mailcal.Services;

/// <summary>
/// The editor chrome's localised strings, in the shape <c>window.setComposerLabels</c> takes.
/// </summary>
/// <remarks>
/// The shared bundle ships English and every host is expected to send its own translations; a host
/// that never calls the hook silently keeps English, which is what this client did. The keys are
/// the bundle's own (<c>clients/composer/src/labels.ts</c>) and the values come from the shared
/// catalog, so the toolbar follows the app's language like every other surface.
///
/// Both editor hosts use this, the composer and the Settings signature editor, because they load
/// the same bundle.
///
/// Both halves of a key mismatch fail silently: a key the bundle does not know is dropped by its
/// <c>mergeLabels</c>, and one it knows but this omits keeps its English default. Nothing throws and
/// nothing logs, so the set is pinned across the clients by
/// <c>scripts/ci/check_composer_labels.py</c>. It cannot live in <c>Mailcal.Tests</c>: that project
/// is plain <c>net10.0</c> and <c>L10n</c> needs a Windows TFM.
/// </remarks>
internal static class ComposerLabels
{
    /// <summary>Every key the bundle's <c>Labels</c> declares, keyed exactly as it spells them.</summary>
    internal static IReadOnlyDictionary<string, string> Values() => new Dictionary<string, string>
    {
        ["placeholder"] = L10n.EditorPlaceholder(),
        ["bold"] = L10n.EditorBold(),
        ["italic"] = L10n.EditorItalic(),
        ["underline"] = L10n.EditorUnderline(),
        ["fontSize"] = L10n.EditorFontSize(),
        ["sizeNormal"] = L10n.EditorSizeNormal(),
        ["sizeSmall"] = L10n.EditorSizeSmall(),
        ["sizeLarge"] = L10n.EditorSizeLarge(),
        ["sizeHuge"] = L10n.EditorSizeHuge(),
        ["bulletedList"] = L10n.EditorBulletedList(),
        ["numberedList"] = L10n.EditorNumberedList(),
        ["indent"] = L10n.EditorIndent(),
        ["outdent"] = L10n.EditorOutdent(),
        ["textColour"] = L10n.EditorTextColour(),
        ["colourAutomatic"] = L10n.EditorColourAutomatic(),
        ["highlight"] = L10n.EditorHighlight(),
        ["highlightNone"] = L10n.EditorHighlightNone(),
        ["table"] = L10n.EditorTable(),
        ["insertTable"] = L10n.EditorInsertTable(),
        ["insertRowAbove"] = L10n.EditorInsertRowAbove(),
        ["insertRowBelow"] = L10n.EditorInsertRowBelow(),
        ["insertColumnLeft"] = L10n.EditorInsertColumnLeft(),
        ["insertColumnRight"] = L10n.EditorInsertColumnRight(),
        ["deleteRow"] = L10n.EditorDeleteRow(),
        ["deleteColumn"] = L10n.EditorDeleteColumn(),
        ["deleteTable"] = L10n.EditorDeleteTable(),
    };

    /// <summary>The call the hosts inject once the bundle has parsed. The serializer is what makes
    /// this safe to interpolate: a translation holding a quote or a backslash is escaped by it.</summary>
    internal static string Script() => $"window.setComposerLabels({JsonSerializer.Serialize(Values())})";
}
