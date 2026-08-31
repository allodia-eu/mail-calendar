// The editor chrome's strings.
//
// The bundle ships English (the markup and the defaults below) so a host that never calls
// `setComposerLabels` still reads sensibly; each client passes its own translations from the shared
// l10n catalog. The core carries no runtime locale and the editor is one asset shared across
// platforms, so the strings cannot be baked per-language here.
//
// Every field is optional: a missing one keeps the English default. The font-size `<option>`
// *values* are never touched: they are the `FontSize` tokens the document is keyed on; only their
// visible text changes.

export interface Labels {
  placeholder: string;
  bold: string;
  italic: string;
  underline: string;
  fontSize: string;
  sizeNormal: string;
  sizeSmall: string;
  sizeLarge: string;
  sizeHuge: string;
  bulletedList: string;
  numberedList: string;
  indent: string;
  outdent: string;
  textColour: string;
  colourAutomatic: string;
  highlight: string;
  highlightNone: string;
  table: string;
  insertTable: string;
  insertRowAbove: string;
  insertRowBelow: string;
  insertColumnLeft: string;
  insertColumnRight: string;
  deleteRow: string;
  deleteColumn: string;
  deleteTable: string;
}

export const DEFAULT_LABELS: Labels = {
  placeholder: "Write your message",
  bold: "Bold",
  italic: "Italic",
  underline: "Underline",
  fontSize: "Font size",
  sizeNormal: "Normal",
  sizeSmall: "Small",
  sizeLarge: "Large",
  sizeHuge: "Huge",
  bulletedList: "Bulleted list",
  numberedList: "Numbered list",
  indent: "Increase indent",
  outdent: "Decrease indent",
  textColour: "Text colour",
  colourAutomatic: "Automatic",
  highlight: "Highlight",
  highlightNone: "No highlight",
  table: "Table",
  insertTable: "Insert table",
  insertRowAbove: "Insert row above",
  insertRowBelow: "Insert row below",
  insertColumnLeft: "Insert column left",
  insertColumnRight: "Insert column right",
  deleteRow: "Delete row",
  deleteColumn: "Delete column",
  deleteTable: "Delete table",
};

/// Merges a host's (possibly partial, possibly JSON-encoded) label set over the defaults, dropping
/// anything that is not a non-empty string; a host that sends a null for one key keeps English for
/// it rather than blanking the control.
export function mergeLabels(current: Labels, incoming: unknown): Labels {
  const source: Record<string, unknown> =
    typeof incoming === "string" ? safeParse(incoming) : ((incoming as Record<string, unknown>) ?? {});
  const merged = { ...current };
  for (const key of Object.keys(DEFAULT_LABELS) as (keyof Labels)[]) {
    const value = source[key];
    if (typeof value === "string" && value.length > 0) merged[key] = value;
  }
  return merged;
}

function safeParse(json: string): Record<string, unknown> {
  try {
    const parsed: unknown = JSON.parse(json);
    return parsed && typeof parsed === "object" ? (parsed as Record<string, unknown>) : {};
  } catch {
    return {};
  }
}
