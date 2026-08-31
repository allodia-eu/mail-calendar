import { describe, expect, test } from "bun:test";

import { documentBlocks } from "../src/document";
import {
  cellAtCaret,
  deleteColumn,
  deleteRow,
  deleteTable,
  insertColumn,
  insertRow,
  insertTable,
  moveToAdjacentCell,
} from "../src/tables";
import { harness } from "./support";

/// A 2x2 table with addressable cells: r<row>c<column>.
const GRID =
  "<table><tbody>" +
  '<tr><td id=r0c0>a</td><td id=r0c1>b</td></tr>' +
  '<tr><td id=r1c0>c</td><td id=r1c1>d</td></tr>' +
  "</tbody></table>";

/// The table's shape as `rows x columns`, read the way Rust's validator reads it.
function shape(editor: HTMLElement): string {
  const block = documentBlocks(editor).find((b) => "Table" in b) as
    | { Table: { rows: { cells: unknown[] }[] } }
    | undefined;
  if (!block) return "none";
  const widths = new Set(block.Table.rows.map((row) => row.cells.length));
  return `${block.Table.rows.length}x${[...widths].join("|")}`;
}

describe("insert", () => {
  test("replaces a blank line rather than pushing it down", () => {
    const h = harness("<p id=p><br></p>");
    h.caret("#p");
    insertTable(h.editor, 2, 3);
    expect(h.editor.querySelector("p#p")).toBeNull();
    expect(shape(h.editor)).toBe("2x3");
  });

  test("lands after a line that has text, and leaves a paragraph to type into below", () => {
    const h = harness("<p id=p>hello</p>");
    h.caret("#p");
    insertTable(h.editor, 1, 2);
    const children = Array.from(h.editor.children).map((c) => c.tagName);
    expect(children).toEqual(["P", "TABLE", "P"]);
  });

  test("clamps a degenerate size to a real table", () => {
    const h = harness("<p id=p><br></p>");
    h.caret("#p");
    insertTable(h.editor, 0, 0);
    expect(shape(h.editor)).toBe("1x1");
  });

  test("from inside a cell it lands after the table, not nested and not at the end", () => {
    // A cell holds inline content only, so the new table cannot go where the caret is. Anchoring it
    // to the enclosing table is the hoist an `<li>` already gets; the alternative is the document's
    // end, which on a reply is below the quoted original and the signature.
    const h = harness(`${GRID}<p id=after>tail</p>`);
    h.caret("#r0c0");
    insertTable(h.editor, 1, 1);
    const children = Array.from(h.editor.children).map((c) => c.tagName);
    expect(children).toEqual(["TABLE", "TABLE", "P", "P"]);
    expect(h.editor.querySelector("td table")).toBeNull();
  });

  test("from a nested list item it clears the whole list, not just the inner one", () => {
    // An <li> holds inline content and a sub-list and nothing else, so a table anchored to the
    // NEAREST list of a nested item lands inside the parent <li>: where `documentBlocks` flattens
    // it into that item's text and the table never reaches the message.
    const h = harness("<ul><li>a<ul><li id=b>b</li></ul></li></ul>");
    h.caret("#b");
    insertTable(h.editor, 2, 2);
    expect(h.editor.querySelector("li table")).toBeNull();
    expect(Array.from(h.editor.children).map((c) => c.tagName)).toEqual(["UL", "TABLE", "P"]);
    expect(shape(h.editor)).toBe("2x2");
  });

  test("a cell wrapping its content in a div still hoists out of the table", () => {
    // The nearest block ancestor of the caret is then the <div>, whose parent is the cell: so a
    // hoist that looked for a block first would insert the table inside the cell after all.
    const h = harness("<table><tbody><tr><td id=c><div id=d>x</div></td></tr></tbody></table>");
    h.caret("#d");
    insertTable(h.editor, 1, 1);
    expect(h.editor.querySelector("td table")).toBeNull();
    expect(Array.from(h.editor.children).map((c) => c.tagName)).toEqual(["TABLE", "TABLE", "P"]);
  });
});

describe("rows and columns", () => {
  test("adds a row below with the same width", () => {
    const h = harness(GRID);
    insertRow(h.editor, h.caret("#r0c0"), true);
    expect(shape(h.editor)).toBe("3x2");
  });

  test("adds a column in every row, keeping the table rectangular", () => {
    const h = harness(GRID);
    insertColumn(h.editor, h.caret("#r0c0"), true);
    expect(shape(h.editor)).toBe("2x3");
    expect(h.html()).toContain('<td id="r0c0">a</td><td><br></td><td id="r0c1">b</td>');
  });

  test("adds a column to the left of the caret's cell", () => {
    const h = harness(GRID);
    insertColumn(h.editor, h.caret("#r0c1"), false);
    expect(h.html()).toContain('<td id="r0c0">a</td><td><br></td><td id="r0c1">b</td>');
  });

  test("deletes a row", () => {
    const h = harness(GRID);
    deleteRow(h.editor, h.caret("#r0c0"));
    expect(shape(h.editor)).toBe("1x2");
    expect(h.editor.querySelector("#r0c0")).toBeNull();
  });

  test("deletes a column from every row", () => {
    const h = harness(GRID);
    deleteColumn(h.editor, h.caret("#r0c1"));
    expect(shape(h.editor)).toBe("2x1");
    expect(h.editor.querySelector("#r1c1")).toBeNull();
  });
});

describe("the last row or column takes the table with it", () => {
  test("deleting the only row removes the table, not leaving an empty one", () => {
    const h = harness("<table><tbody><tr><td id=only>x</td></tr></tbody></table>");
    deleteRow(h.editor, h.caret("#only"));
    expect(shape(h.editor)).toBe("none");
    expect(h.html()).toBe("<p><br></p>");
  });

  test("deleting the only column removes the table", () => {
    const h = harness(
      "<table><tbody><tr><td id=a>x</td></tr><tr><td id=b>y</td></tr></tbody></table>",
    );
    deleteColumn(h.editor, h.caret("#a"));
    expect(shape(h.editor)).toBe("none");
  });

  test("deleting the table leaves a paragraph to type into", () => {
    const h = harness(GRID);
    deleteTable(h.editor, h.caret("#r0c0"));
    expect(h.html()).toBe("<p><br></p>");
  });
});

describe("caret", () => {
  test("Tab walks to the next cell and stops at the end of the table", () => {
    const h = harness(GRID);
    h.caret("#r0c0");
    expect(moveToAdjacentCell(h.editor, false)).toBe(true);
    expect(cellAtCaret(h.editor)?.id).toBe("r0c1");
    h.caret("#r1c1");
    expect(moveToAdjacentCell(h.editor, false)).toBe(false);
  });

  test("Shift+Tab walks back and stops at the first cell", () => {
    const h = harness(GRID);
    h.caret("#r0c1");
    expect(moveToAdjacentCell(h.editor, true)).toBe(true);
    expect(cellAtCaret(h.editor)?.id).toBe("r0c0");
    expect(moveToAdjacentCell(h.editor, true)).toBe(false);
  });

  test("a nested table's rows never leak into the outer table's shape", () => {
    const h = harness(
      "<table><tbody><tr><td id=outer>" +
        "<table><tbody><tr><td>inner</td></tr><tr><td>inner2</td></tr></tbody></table>" +
        "</td><td>b</td></tr></tbody></table>",
    );
    h.caret("#outer");
    // The outer table is one row of two cells whatever the inner one contains.
    const outer = documentBlocks(h.editor).find((b) => "Table" in b) as {
      Table: { rows: { cells: unknown[] }[] };
    };
    expect(outer.Table.rows.length).toBe(1);
    expect(outer.Table.rows[0]!.cells.length).toBe(2);
  });
});

describe("the shape commands require a cell at compile time", () => {
  // Not a runtime assertion: `@ts-expect-error` fails the typecheck when the error it claims does
  // NOT happen, so this is what keeps `ancestorOf`'s tag-name inference honest. Without it the
  // narrow signatures could widen back to HTMLElement and every test here would still pass, since
  // `locate()` rejects a non-cell at runtime anyway and these commands would just return false.
  test("a paragraph is not a table cell", () => {
    const h = harness("<p id=p>not a cell</p>");
    const paragraph = h.caret<HTMLParagraphElement>("#p");
    // @ts-expect-error a paragraph is not an HTMLTableCellElement
    expect(deleteRow(h.editor, paragraph)).toBe(false);
    // @ts-expect-error a paragraph is not an HTMLTableCellElement
    expect(insertColumn(h.editor, paragraph, true)).toBe(false);
  });
});
