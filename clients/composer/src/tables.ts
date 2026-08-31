// Inserting a table and editing its shape.
//
// Every operation here preserves two invariants Rust enforces on submit (`validate_table`): a table
// has at least one row, and every row has the same, non-zero cell count. A ragged table is not a
// cosmetic bug; it fails the send. So deleting the last row or the last column removes the whole
// table rather than leaving a degenerate one behind.

import { ancestorAtCaret, ancestorOf, caretInto, documentOf, emptyLine, isBlank, rangeWithin } from "./dom";
import { tableRows } from "./document";

/// The cells of one row; direct children only, so a table nested in a cell contributes none of its
/// own to this row's count.
function cellsOf(row: Element): HTMLTableCellElement[] {
  return Array.from(row.children).filter(
    (cell) => cell.tagName === "TD" || cell.tagName === "TH",
  ) as HTMLTableCellElement[];
}

function newCell(doc: Document): HTMLTableCellElement {
  const cell = doc.createElement("td");
  cell.appendChild(doc.createElement("br"));
  return cell;
}

/// The table cell the caret is in, or `null` when it is not in one. Every shape command takes this,
/// so a toolbar button pressed with the caret outside a table is a no-op rather than an edit
/// somewhere unexpected.
export function cellAtCaret(editor: HTMLElement): HTMLTableCellElement | null {
  return ancestorAtCaret(editor, "td", "th");
}

/// The table that owns `cell`: its nearest table ancestor, bounded by the editor.
function tableOf(cell: HTMLTableCellElement, editor: HTMLElement): HTMLTableElement | null {
  return ancestorOf(cell.parentElement, editor, "table");
}

/// The OUTERMOST list `li` belongs to, bounded by the editor.
///
/// Outermost, not nearest: an `<li>` holds inline content and a sub-list and nothing else, so a
/// table anchored to the nearest list of a *nested* item lands inside the parent `<li>`: where
/// `documentBlocks` flattens it into that item's text and the table never reaches the message.
/// Only the top-level list has a home for it beside itself.
function outermostList(li: HTMLElement, editor: HTMLElement): HTMLElement | null {
  let found: HTMLElement | null = null;
  for (let node: HTMLElement | null = li; node && node !== editor; node = node.parentElement) {
    if (node.tagName === "UL" || node.tagName === "OL") found = node;
  }
  return found;
}

interface Position {
  table: HTMLTableElement;
  rows: Element[];
  row: Element;
  rowIndex: number;
  columnIndex: number;
}

function locate(cell: HTMLTableCellElement, editor: HTMLElement): Position | null {
  const table = tableOf(cell, editor);
  if (!table) return null;
  const rows = tableRows(table);
  const row = cell.parentElement;
  if (!row) return null;
  const rowIndex = rows.indexOf(row);
  const columnIndex = cellsOf(row).indexOf(cell);
  if (rowIndex < 0 || columnIndex < 0) return null;
  return { table, rows, row, rowIndex, columnIndex };
}

/// Builds a `rows` × `columns` table and puts it in the document at the caret.
///
/// A blank line is replaced rather than pushed down: pressing Table on an empty paragraph should
/// put the table where the caret is, not leave an orphaned blank line above it. The trailing empty
/// paragraph is what lets the user carry on typing below a table that would otherwise be the last
/// thing in the body with nowhere to click.
export function insertTable(editor: HTMLElement, rows: number, columns: number): HTMLTableElement | null {
  const doc = documentOf(editor);
  const rowCount = Math.max(1, Math.floor(rows));
  const columnCount = Math.max(1, Math.floor(columns));

  const table = doc.createElement("table");
  const body = doc.createElement("tbody");
  for (let r = 0; r < rowCount; r += 1) {
    const row = doc.createElement("tr");
    for (let c = 0; c < columnCount; c += 1) row.appendChild(newCell(doc));
    body.appendChild(row);
  }
  table.appendChild(body);

  const trailing = emptyLine(doc);
  const range = rangeWithin(editor);
  // A cell holds inline content only, so a table cannot nest inside one; the caret being in a cell
  // anchors the new table after the table that holds it. Same hoist an `<li>` gets to its list
  // below, and it has to be looked for first: a cell containing a `<div>` would otherwise resolve to
  // that div and put the new table inside the cell after all.
  const cell = range ? ancestorOf(range.startContainer, editor, "td", "th") : null;
  const enclosing = cell ? ancestorOf(cell, editor, "table") : null;
  const block = range && !cell
    ? ancestorOf(range.startContainer, editor, "p", "div", "li")
    : null;

  if (enclosing?.parentElement) {
    enclosing.parentElement.insertBefore(table, enclosing.nextSibling);
    table.parentElement?.insertBefore(trailing, table.nextSibling);
  } else if (block && block.tagName !== "LI" && isBlank(block)) {
    block.replaceWith(table);
    table.parentElement?.insertBefore(trailing, table.nextSibling);
  } else if (block && block.parentElement) {
    const anchor = block.tagName === "LI" ? outermostList(block, editor) ?? block : block;
    anchor.parentElement?.insertBefore(table, anchor.nextSibling);
    table.parentElement?.insertBefore(trailing, table.nextSibling);
  } else {
    editor.appendChild(table);
    editor.appendChild(trailing);
  }

  const first = table.querySelector("td");
  if (first) caretInto(first);
  return table;
}

/// Adds a row above or below the one holding `cell`, with the same number of cells.
export function insertRow(editor: HTMLElement, cell: HTMLTableCellElement, below: boolean): boolean {
  const at = locate(cell, editor);
  if (!at) return false;
  const doc = documentOf(cell);
  const width = cellsOf(at.row).length;
  const row = doc.createElement("tr");
  for (let c = 0; c < width; c += 1) row.appendChild(newCell(doc));
  at.row.parentElement?.insertBefore(row, below ? at.row.nextSibling : at.row);
  caretInto(row.firstElementChild ?? row);
  return true;
}

/// Adds a column left or right of the one holding `cell`, in **every** row: the table stays
/// rectangular, which is what Rust requires.
export function insertColumn(editor: HTMLElement, cell: HTMLTableCellElement, after: boolean): boolean {
  const at = locate(cell, editor);
  if (!at) return false;
  const doc = documentOf(cell);
  let focus: HTMLElement | null = null;
  for (const row of at.rows) {
    const cells = cellsOf(row);
    const fresh = newCell(doc);
    const anchor = cells[at.columnIndex];
    if (anchor) row.insertBefore(fresh, after ? anchor.nextSibling : anchor);
    else row.appendChild(fresh);
    if (row === at.row) focus = fresh;
  }
  if (focus) caretInto(focus);
  return true;
}

/// Removes the row holding `cell`. Removing the last row removes the table: an empty table fails
/// validation, and a table with no rows is not something the user can see or click back into.
export function deleteRow(editor: HTMLElement, cell: HTMLTableCellElement): boolean {
  const at = locate(cell, editor);
  if (!at) return false;
  if (at.rows.length <= 1) return deleteTable(editor, cell);
  const fallback = at.rows[at.rowIndex + 1] ?? at.rows[at.rowIndex - 1];
  at.row.remove();
  const next = fallback ? cellsOf(fallback)[at.columnIndex] ?? cellsOf(fallback)[0] : null;
  if (next) caretInto(next);
  return true;
}

/// Removes the column holding `cell` from every row. Removing the last column removes the table,
/// for the same reason `deleteRow` does.
export function deleteColumn(editor: HTMLElement, cell: HTMLTableCellElement): boolean {
  const at = locate(cell, editor);
  if (!at) return false;
  if (cellsOf(at.row).length <= 1) return deleteTable(editor, cell);
  let focus: HTMLElement | null = null;
  for (const row of at.rows) {
    const cells = cellsOf(row);
    const doomed = cells[at.columnIndex];
    const survivor = cells[at.columnIndex + 1] ?? cells[at.columnIndex - 1] ?? null;
    if (row === at.row) focus = survivor;
    doomed?.remove();
  }
  if (focus) caretInto(focus);
  return true;
}

/// Removes the whole table, leaving a paragraph in its place so the caret has somewhere to land.
export function deleteTable(editor: HTMLElement, cell: HTMLTableCellElement): boolean {
  const table = tableOf(cell, editor);
  if (!table) return false;
  const paragraph = emptyLine(documentOf(table));
  table.replaceWith(paragraph);
  caretInto(paragraph);
  return true;
}

/// Moves the caret to the next (or previous) cell of the table it is in. Returns false when there
/// is no such cell, so Tab can fall through to its other meanings at the table's edges.
export function moveToAdjacentCell(editor: HTMLElement, back: boolean): boolean {
  const cell = cellAtCaret(editor);
  if (!cell) return false;
  const table = tableOf(cell, editor);
  if (!table) return false;
  const cells = tableRows(table).flatMap((row) => cellsOf(row));
  const next = cells[cells.indexOf(cell) + (back ? -1 : 1)];
  if (!next) return false;
  caretInto(next);
  return true;
}
