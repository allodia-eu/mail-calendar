import { describe, expect, test } from "bun:test";

import { documentBlocks } from "../src/document";
import { pastedHtml } from "../src/paste";
import type { Block, InlineContent, TextRun } from "../src/types";
import { harness } from "./support";

/// What a paste of `html` into an empty draft leaves in the document.
function pasted(html: string): Block[] {
  const { editor } = harness("");
  const markup = pastedHtml(editor.ownerDocument, html);
  if (markup === null) throw new Error("nothing was kept");
  editor.innerHTML = markup;
  return documentBlocks(editor);
}

function runs(content: InlineContent[]): TextRun[] {
  return content.flatMap((inline) => ("Text" in inline ? [inline.Text] : []));
}

function paragraphs(blocks: Block[]): TextRun[][] {
  return blocks.map((block) => ("Paragraph" in block ? runs(block.Paragraph.content) : []));
}

function text(content: InlineContent[]): string {
  return runs(content)
    .map((run) => run.text)
    .join("");
}

// The shape Word for Mac writes to the clipboard: a style on every paragraph and run, a colour and
// a background nobody chose, `<o:p>`, `&nbsp;` either side of a bold word, an empty line as
// `<o:p>&nbsp;</o:p>`, and source lines wrapped in the middle of a sentence.
const WORD = `<html xmlns:o="urn:schemas-microsoft-com:office:office"><head>
<meta charset="utf-8"><style>p.MsoNormal { margin: 0cm; font-size: 12pt; }</style></head>
<body lang=EN-US link="#467886" style='tab-interval:36.0pt;word-wrap:break-word'>
<!--StartFragment-->

<p class=MsoNormal style='line-height:normal;background:white'><span style='font-size:11.5pt;
font-family:"Segoe UI",sans-serif;color:#1A1A1A'>Your order&nbsp;<b>1234</b>&nbsp;has
shipped and should arrive this week.<o:p></o:p></span></p>

<p class=MsoNormal style='line-height:normal;background:white'><b><span style='font-size:11.5pt;
color:#1A1A1A'>Note: delivery needs a signature</span></b><span style='color:#1A1A1A'><o:p></o:p></span></p>

<p class=MsoNormal style='line-height:normal;background:white'><span style='color:#1A1A1A'>Track it
on <a
href="https://example.com/track">the carrier's page</a>, any time.<o:p></o:p></span></p>

<p class=MsoNormal style='margin-bottom:0cm;background:white'><span style='color:#1A1A1A'><o:p>&nbsp;</o:p></span></p>

<p class=MsoNormal style='margin-bottom:0cm;background:white'><span style='color:#1A1A1A'>Kind
regards,<o:p></o:p></span></p>

<!--EndFragment-->
</body></html>`;

describe("pasting from Word", () => {
  test("keeps the paragraphs, the bold and the link, and nothing Word stamped on every run", () => {
    const blocks = paragraphs(pasted(WORD));
    expect(blocks.map((line) => line.map((run) => run.text).join(""))).toEqual([
      "Your order 1234 has shipped and should arrive this week.",
      "Note: delivery needs a signature",
      "Track it on the carrier's page, any time.",
      " ",
      "Kind regards,",
    ]);

    const order = blocks[0]!;
    expect(order.filter((run) => run.bold).map((run) => run.text)).toEqual(["1234"]);

    expect(blocks[1]!.every((run) => run.bold)).toBe(true);

    const link = blocks[2]!.find((run) => run.link);
    expect(link).toMatchObject({ text: "the carrier's page", link: "https://example.com/track" });
    // A link is drawn underlined; the mark would send it out underlined twice.
    expect(link!.underline).toBe(false);

    for (const run of blocks.flat()) {
      expect(run.color).toBeUndefined();
      expect(run.highlight).toBeUndefined();
      expect(run.font_size).toBeUndefined();
    }
  });
});

describe("pasted marks", () => {
  test("a style can take bold away as well as add it", () => {
    // Google Docs wraps a whole copy in a `<b>` that is not bold, and bolds with a style.
    const [line] = paragraphs(
      pasted(
        '<b style="font-weight:normal" id="docs-internal-guid-1"><p dir="ltr">' +
          '<span style="font-weight:400">plain </span><span style="font-weight:700">bold </span>' +
          '<span style="font-style:italic">italic </span>' +
          '<span style="text-decoration:underline">underlined</span></p></b>',
      ),
    );
    expect(line!.map((run) => [run.text, run.bold, run.italic, run.underline])).toEqual([
      ["plain ", false, false, false],
      ["bold ", true, false, false],
      ["italic ", false, true, false],
      ["underlined", false, false, true],
    ]);
  });

  test("the editor's own colour, highlight and size stamps survive; a foreign colour does not", () => {
    const [line] = paragraphs(
      pasted(
        '<p><span data-color="#c00000" style="color:#c00000">red</span> ' +
          '<span data-highlight="#ffff00" data-size="Large">marked</span> ' +
          '<span style="color:#ffffff;background-color:#000">foreign</span></p>',
      ),
    );
    expect(line!.find((run) => run.text === "red")?.color).toBe("#c00000");
    expect(line!.find((run) => run.text === "marked")).toMatchObject({
      highlight: "#ffff00",
      font_size: "Large",
    });
    const foreign = line!.find((run) => run.text.includes("foreign"))!;
    expect(foreign.color).toBeUndefined();
    expect(foreign.highlight).toBeUndefined();
  });

  test("a heading arrives as a bold line", () => {
    const blocks = paragraphs(pasted("<h2>Agenda</h2><p>Monday</p>"));
    expect(blocks.map((line) => line.map((run) => [run.text, run.bold]))).toEqual([
      [["Agenda", true]],
      [["Monday", false]],
    ]);
  });
});

describe("pasted markup that is not kept", () => {
  test("scripts, styles, pictures, hidden text and form controls leave nothing behind", () => {
    const { editor } = harness("");
    const markup = pastedHtml(
      editor.ownerDocument,
      '<style>p{color:red}</style><script>alert(1)</script>' +
        '<p onclick="alert(2)" class="x">Hello <img src="https://tracker.example/p.gif" onerror="alert(3)">' +
        '<span style="display:none">preheader</span><span hidden>secret</span>' +
        '<input value="typed"><iframe src="https://example.com"></iframe>world</p>',
    )!;
    expect(markup).toBe("Hello world");
  });

  test("a link this product does not send keeps its text and loses its target", () => {
    const [line] = paragraphs(pasted('<p><a href="javascript:alert(1)">click</a> <a href="#top">top</a></p>'));
    expect(line!.map((run) => [run.text, run.link ?? null])).toEqual([["click top", null]]);
  });

  test("a clipboard with no text yields nothing, so the plain-text flavour is used", () => {
    const { editor } = harness("");
    expect(pastedHtml(editor.ownerDocument, '<img src="https://example.com/a.png">')).toBeNull();
    expect(pastedHtml(editor.ownerDocument, "")).toBeNull();
  });
});

describe("pasted structure", () => {
  test("one paragraph comes back as its runs, to join the sentence at the caret", () => {
    const { editor } = harness("");
    expect(pastedHtml(editor.ownerDocument, "<p>a <b>bold</b> word</p>")).toBe("a <b>bold</b> word");
  });

  test("a line break inside a paragraph stays a line break", () => {
    const blocks = paragraphs(pasted("<p>Alice<br>Main Street 1</p>"));
    expect(blocks.map((line) => line.map((run) => run.text).join(""))).toEqual(["Alice", "Main Street 1"]);
  });

  test("nested lists keep their kind and their depth", () => {
    const [block] = pasted(
      "<ul><li>First<ol><li><b>one</b></li><li>two</li></ol></li><li><p>Second</p></li></ul>",
    );
    if (!block || !("List" in block)) throw new Error("expected a list");
    expect(block.List.kind).toBe("Bullet");
    expect(block.List.items.map((item) => text(item.content))).toEqual(["First", "Second"]);
    const child = block.List.items[0]!.child!;
    expect(child.kind).toBe("Ordered");
    expect(child.items.map((item) => text(item.content))).toEqual(["one", "two"]);
    expect(runs(child.items[0]!.content)[0]!.bold).toBe(true);
  });

  test("a table with a merged cell is padded to a rectangle", () => {
    const [block] = pasted(
      "<table><thead><tr><th>Name</th><th>Role</th></tr></thead>" +
        '<tbody><tr><td colspan="2"><p>Alice</p><p>Bob</p></td></tr></tbody></table>',
    );
    if (!block || !("Table" in block)) throw new Error("expected a table");
    const rows = block.Table.rows.map((row) => row.cells.map((cell) => text(cell.content)));
    expect(rows).toEqual([
      ["Name", "Role"],
      ["Alice Bob", ""],
    ]);
    expect(runs(block.Table.rows[0]!.cells[0]!.content)[0]!.bold).toBe(true);
  });

  test("a table inside a cell is flattened into that cell's text", () => {
    const [block] = pasted(
      "<table><tr><td>outer <table><tr><td>a</td><td>b</td></tr></table></td></tr></table>",
    );
    if (!block || !("Table" in block)) throw new Error("expected a table");
    expect(block.Table.rows.map((row) => row.cells.map((cell) => text(cell.content)))).toEqual([
      ["outer a b"],
    ]);
  });
});
