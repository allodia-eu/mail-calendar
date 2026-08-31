// Wiring the toolbar: the formatting buttons, and the two popovers (colour palette, table menu).

import { documentOf, windowOf } from "./dom";
import { applyColor, applyFontSize, applyMark, type ColorKind } from "./format";
import { indentSelection } from "./lists";
import { type Labels } from "./labels";
import {
  cellAtCaret,
  deleteColumn,
  deleteRow,
  deleteTable,
  insertColumn,
  insertRow,
  insertTable,
} from "./tables";
import { isFontSize } from "./types";

/// Two rows of six. Named by nothing but their value: naming twelve colours would be twelve more
/// catalog keys in seven locales for information the swatch already conveys, and "Automatic" and
/// "No highlight": the two that are not self-evident: are labelled.
const TEXT_COLOURS = [
  "#000000", "#7f7f7f", "#c00000", "#ff0000", "#ffc000", "#ffff00",
  "#92d050", "#00b050", "#00b0f0", "#0070c0", "#002060", "#7030a0",
];

const HIGHLIGHTS = ["#ffff00", "#92d050", "#00ffff", "#ff99cc", "#ffc000", "#c0c0c0"];

/// The largest table the size picker offers. Beyond this the grid stops being quicker than typing.
const PICKER_ROWS = 6;
const PICKER_COLUMNS = 8;

interface Popover {
  button: HTMLElement;
  panel: HTMLElement;
  build: () => void;
}

export interface Toolbar {
  applyLabels(labels: Labels): void;
}

export function installToolbar(editor: HTMLElement, root: HTMLElement, labels: () => Labels): Toolbar {
  const doc = documentOf(root);
  const byId = <T extends HTMLElement>(id: string): T => {
    const node = doc.getElementById(id);
    if (!node) throw new Error(`editor markup is missing #${id}`);
    return node as T;
  };

  // A press anywhere in the toolbar collapses the editor's selection before the click handler runs
  //: a <button> by stealing focus, a plain element (the table picker's cells) by starting a new
  // selection where the pointer went down; so bold/indent/colour would apply to nothing and
  // `insertTable` would find no caret and append the table below the quoted original. Cancelling
  // mousedown keeps focus (and the selection) in the editor. Delegated, so the popovers' contents,
  // built on open: are covered too. The font-size <select> is the one exclusion, so its native
  // dropdown still opens.
  root.addEventListener("mousedown", (event) => {
    const target = event.target as Element | null;
    if (!target?.closest?.("select")) event.preventDefault();
  });

  for (const button of Array.from(root.querySelectorAll<HTMLElement>("[data-command]"))) {
    button.addEventListener("click", () => {
      const command = button.dataset.command;
      if (command === "bold" || command === "italic" || command === "underline") {
        applyMark(editor, command);
      }
    });
  }

  const fontSize = byId<HTMLSelectElement>("font-size");
  fontSize.addEventListener("change", () => {
    if (isFontSize(fontSize.value)) applyFontSize(editor, fontSize.value);
  });

  byId("bullet-list").addEventListener("click", () => legacy(editor, "insertUnorderedList"));
  byId("ordered-list").addEventListener("click", () => legacy(editor, "insertOrderedList"));
  byId("indent").addEventListener("click", () => indentSelection(editor, false));
  byId("outdent").addEventListener("click", () => indentSelection(editor, true));

  // --- Popovers ---

  // Which edge a popover hangs from, decided when it opens rather than baked into the markup.
  //
  // The toolbar WRAPS: in a narrow composer; macOS's detail column, a split Windows pane; the
  // last controls drop to a second row and start again at the left. A button that sits at the right
  // edge in a wide window is at the LEFT edge in a narrow one, so a fixed alignment is wrong half
  // the time, and wrong here means the menu opens outside the WebView and is clipped away: no
  // scrollbar, no overflow, just gone.
  //
  // Measured against the editor's own viewport, and only flipped when it does not fit, so the
  // default stays left-aligned under its button.
  const alignPopover = (popover: Popover) => {
    popover.panel.classList.remove("align-end");
    const view = windowOf(editor);
    const width = view.innerWidth;
    // A hidden panel measures 0; reveal it for the measurement and restore, so the class is decided
    // before anything paints.
    const wasHidden = popover.panel.hidden;
    popover.panel.hidden = false;
    const button = popover.button.getBoundingClientRect();
    const panel = popover.panel.getBoundingClientRect();
    popover.panel.hidden = wasHidden;
    if (width > 0 && panel.width > 0 && button.left + panel.width > width) {
      popover.panel.classList.add("align-end");
    }
  };

  const popovers: Popover[] = [];
  const closeAll = (except?: Popover) => {
    for (const popover of popovers) {
      if (popover === except) continue;
      popover.panel.hidden = true;
      popover.button.setAttribute("aria-expanded", "false");
    }
  };

  const register = (buttonId: string, panelId: string, build: (panel: HTMLElement) => void) => {
    const popover: Popover = {
      button: byId(buttonId),
      panel: byId(panelId),
      build: () => build(byId(panelId)),
    };
    popovers.push(popover);
    popover.button.addEventListener("click", () => {
      const opening = popover.panel.hidden;
      closeAll(popover);
      if (opening) {
        popover.build();
        alignPopover(popover);
      }
      popover.panel.hidden = !opening;
      popover.button.setAttribute("aria-expanded", String(opening));
    });
    return popover;
  };

  const swatchBar = (id: string, colour: string) => {
    byId(id).style.background = colour;
  };

  const paletteBuilder =
    (kind: ColorKind, colours: string[], barId: string, resetLabel: () => string) =>
    (panel: HTMLElement) => {
      panel.textContent = "";
      const grid = doc.createElement("div");
      grid.className = "swatches";
      for (const colour of colours) {
        const swatch = doc.createElement("button");
        swatch.type = "button";
        swatch.style.background = colour;
        swatch.title = colour;
        swatch.setAttribute("aria-label", colour);
        swatch.addEventListener("click", () => {
          applyColor(editor, kind, colour);
          swatchBar(barId, colour);
          closeAll();
        });
        grid.appendChild(swatch);
      }
      panel.appendChild(grid);

      const reset = doc.createElement("button");
      reset.type = "button";
      reset.className = "reset";
      reset.textContent = resetLabel();
      reset.addEventListener("click", () => {
        applyColor(editor, kind, null);
        closeAll();
      });
      panel.appendChild(reset);
    };

  register(
    "text-colour",
    "text-colour-menu",
    paletteBuilder("color", TEXT_COLOURS, "text-colour-bar", () => labels().colourAutomatic),
  );
  register(
    "highlight",
    "highlight-menu",
    paletteBuilder("highlight", HIGHLIGHTS, "highlight-bar", () => labels().highlightNone),
  );
  swatchBar("highlight-bar", HIGHLIGHTS[0]!);

  register("table", "table-menu", (panel) => buildTableMenu(panel, editor, doc, labels(), closeAll));

  // Clicking away or pressing Escape closes an open popover; without this the palette would stay
  // over the message the user just went back to writing.
  doc.addEventListener("mousedown", (event) => {
    if (!(event.target as Element | null)?.closest?.(".menu")) closeAll();
  });
  doc.addEventListener("keydown", (event) => {
    if ((event as KeyboardEvent).key === "Escape") closeAll();
  });

  return {
    applyLabels(current: Labels) {
      editor.dataset.placeholder = current.placeholder;
      editor.setAttribute("aria-label", current.placeholder);
      const title = (selector: string, text: string) => {
        const node = root.querySelector<HTMLElement>(selector);
        if (node) node.title = text;
      };
      title('[data-command="bold"]', current.bold);
      title('[data-command="italic"]', current.italic);
      title('[data-command="underline"]', current.underline);
      title("#bullet-list", current.bulletedList);
      title("#ordered-list", current.numberedList);
      title("#indent", current.indent);
      title("#outdent", current.outdent);
      title("#text-colour", current.textColour);
      title("#highlight", current.highlight);
      title("#table", current.table);
      fontSize.title = current.fontSize;
      fontSize.setAttribute("aria-label", current.fontSize);
      const sizeText: Record<string, string> = {
        Normal: current.sizeNormal,
        Small: current.sizeSmall,
        Large: current.sizeLarge,
        Huge: current.sizeHuge,
      };
      for (const option of Array.from(fontSize.options)) {
        const text = sizeText[option.value];
        if (text) option.textContent = text;
      }
      closeAll();
    },
  };
}

/// The size picker, plus the shape commands when the caret is in a table. Rebuilt on every open, so
/// the commands appear exactly when they can do something rather than sitting there greyed out.
function buildTableMenu(
  panel: HTMLElement,
  editor: HTMLElement,
  doc: Document,
  labels: Labels,
  close: () => void,
): void {
  panel.textContent = "";

  const picker = doc.createElement("div");
  picker.className = "grid-picker";
  const size = doc.createElement("p");
  size.className = "grid-size";
  size.textContent = labels.insertTable;

  const cells: HTMLElement[] = [];
  const paint = (rows: number, columns: number) => {
    cells.forEach((cell, index) => {
      const row = Math.floor(index / PICKER_COLUMNS);
      const column = index % PICKER_COLUMNS;
      cell.classList.toggle("on", row < rows && column < columns);
    });
    size.textContent = rows > 0 ? `${rows} × ${columns}` : labels.insertTable;
  };

  for (let row = 0; row < PICKER_ROWS; row += 1) {
    for (let column = 0; column < PICKER_COLUMNS; column += 1) {
      const cell = doc.createElement("span");
      cell.addEventListener("mouseenter", () => paint(row + 1, column + 1));
      cell.addEventListener("click", () => {
        insertTable(editor, row + 1, column + 1);
        close();
      });
      cells.push(cell);
      picker.appendChild(cell);
    }
  }
  picker.addEventListener("mouseleave", () => paint(0, 0));
  panel.append(picker, size);

  const cell = cellAtCaret(editor);
  if (!cell) return;

  panel.appendChild(doc.createElement("hr"));
  const items = doc.createElement("div");
  items.className = "menu-items";
  const action = (text: string, run: () => void) => {
    const button = doc.createElement("button");
    button.type = "button";
    button.textContent = text;
    button.addEventListener("click", () => {
      run();
      close();
    });
    items.appendChild(button);
  };
  action(labels.insertRowAbove, () => insertRow(editor, cell, false));
  action(labels.insertRowBelow, () => insertRow(editor, cell, true));
  action(labels.insertColumnLeft, () => insertColumn(editor, cell, false));
  action(labels.insertColumnRight, () => insertColumn(editor, cell, true));
  items.appendChild(doc.createElement("hr"));
  action(labels.deleteRow, () => deleteRow(editor, cell));
  action(labels.deleteColumn, () => deleteColumn(editor, cell));
  action(labels.deleteTable, () => deleteTable(editor, cell));
  panel.appendChild(items);
}

/// The list-toggle commands, still `execCommand`: turning a run of paragraphs into a list (and back)
/// is the same selection-splitting problem the mark commands have, and the engines do it correctly.
function legacy(editor: HTMLElement, command: string): void {
  editor.focus({ preventScroll: true });
  (documentOf(editor) as Document & { execCommand?: (c: string, ui?: boolean) => boolean })
    .execCommand?.(command, false);
}
