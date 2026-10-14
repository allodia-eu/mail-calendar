import { describe, expect, test } from "bun:test";

import { documentBlocks } from "../src/document";
import { dropHollowParagraphs, pastedHtml, pasteContent } from "../src/paste";
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

// Excel 365 for Windows' Copy of A1:B3, trimmed: every mark is a class its style sheet defines,
// whole cells through `xl` classes and runs inside one through `<font class>`. The sheet sits inside
// an HTML comment and carries type rules for `td` and `tr` too.
const EXCEL = `<html xmlns:x="urn:schemas-microsoft-com:office:excel"><head>
<meta name=ProgId content=Excel.Sheet>
<style>
<!--table
	{mso-displayed-decimal-separator:"\\.";}
@page
	{margin:.75in .7in .75in .7in;}
.font0
	{color:black;
	font-weight:400;
	font-style:normal;
	text-decoration:none;}
.font5
	{color:black;
	font-weight:700;
	font-style:normal;
	text-decoration:none;}
.font6
	{color:black;
	font-weight:400;
	font-style:italic;
	text-decoration:none;}
td
	{color:black;
	font-weight:400;
	font-style:normal;
	text-decoration:none;
	white-space:nowrap;}
.xl63
	{font-weight:700;}
.xl64
	{font-style:italic;}
.xl65
	{text-decoration:underline;
	text-underline-style:single;}
-->
</style></head>
<body link="#467886" vlink="#96607D">
<table border=0 cellpadding=0 cellspacing=0 width=136 style='border-collapse:collapse;width:102pt'>
<!--StartFragment-->
 <col width=68 span=2 style='width:51pt'>
 <tr height=19 style='height:14.25pt'>
  <td height=19 class=xl63 width=68 style='height:14.25pt;width:51pt'>Name</td>
  <td class=xl63 width=68 style='width:51pt'>Amount</td>
 </tr>
 <tr height=19 style='height:14.25pt'>
  <td height=19 class=xl64 style='height:14.25pt'>Apples</td>
  <td class=xl65 align=right>42</td>
 </tr>
 <tr height=19 style='height:14.25pt'>
  <td height=19 style='height:14.25pt'>Pears, <font class="font5">ripe</font><font
  class="font0"> </font><font class="font6">and</font><font class="font0">
  cheap</font></td>
  <td align=right>7</td>
 </tr>
<!--EndFragment-->
</table>
</body></html>`;

describe("pasting from Excel", () => {
  test("keeps the bold, italic and underline its style sheet gives each cell and run", () => {
    const [block] = pasted(EXCEL);
    if (!block || !("Table" in block)) throw new Error("expected a table");
    const cells = block.Table.rows.map((row) =>
      row.cells.map((cell) =>
        runs(cell.content).map((run) => [run.text, run.bold, run.italic, run.underline]),
      ),
    );
    expect(cells).toEqual([
      [[["Name", true, false, false]], [["Amount", true, false, false]]],
      [[["Apples", false, true, false]], [["42", false, false, true]]],
      [
        [
          ["Pears, ", false, false, false],
          ["ripe", true, false, false],
          [" ", false, false, false],
          ["and", false, true, false],
          [" cheap", false, false, false],
        ],
        [["7", false, false, false]],
      ],
    ]);
  });
});

describe("pasted marks", () => {
  test("a class rule counts as the class's tag names it, and an inline style beats it", () => {
    const [line] = paragraphs(
      pasted(
        "<style>.strong{font-weight:bold} span.slant{font-style:italic} b.slant{font-style:normal}" +
          " .plain, .loud{font-weight:700} .soft{text-decoration-line:underline}</style>" +
          '<p><span class="strong">a </span><span class="strong" style="font-weight:400">b </span>' +
          '<span class="slant">c </span><i class="other slant">d </i><span class="loud">e </span>' +
          '<span class="soft">f</span></p>',
      ),
    );
    expect(line!.map((run) => [run.text, run.bold, run.italic, run.underline])).toEqual([
      ["a ", true, false, false],
      ["b ", false, false, false],
      // `b.slant` does not reach the `<i>`, so both stay italic and form one run.
      ["c d ", false, true, false],
      ["e ", true, false, false],
      ["f", false, false, true],
    ]);
  });

  test("a type rule and a compound selector give nothing: only a class is read", () => {
    const [line] = paragraphs(
      pasted("<style>p{font-weight:700} .a.b{font-style:italic} div .c{font-weight:700}</style>" +
        '<p>one <span class="a b">two</span> <span class="c">three</span></p>'),
    );
    expect(line!.every((run) => !run.bold && !run.italic)).toBe(true);
  });

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
          '<span data-highlight="#ffff00" data-size="18">marked</span> ' +
          '<span style="color:#ffffff;background-color:#000">foreign</span></p>',
      ),
    );
    expect(line!.find((run) => run.text === "red")?.color).toBe("#c00000");
    expect(line!.find((run) => run.text === "marked")).toMatchObject({
      highlight: "#ffff00",
      font_size: 18,
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

describe("after the markup is inserted", () => {
  test("a paragraph holding nothing at all goes, and a blank line the user made stays", () => {
    // Chromium's insertHTML leaves an empty <p></p> after a list or a table followed by a
    // paragraph. It takes no height on screen, and the document would send it as a blank line.
    const { editor, html } = harness("<ul><li>x</li></ul><p></p><p>b</p><p><br></p><p>c</p><p>&nbsp;</p>");
    // The shape Chromium leaves in WebView2: not childless, but holding one empty text node.
    const hollow = editor.ownerDocument.createElement("p");
    hollow.appendChild(editor.ownerDocument.createTextNode(""));
    editor.insertBefore(hollow, editor.querySelector("p:nth-of-type(3)"));
    dropHollowParagraphs(editor);
    expect(html()).toBe("<ul><li>x</li></ul><p>b</p><p><br></p><p>c</p><p>&nbsp;</p>");
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

  test("a merged cell keeps the columns after it in place", () => {
    const [block] = pasted(
      "<table><tr><td>Item</td><td>Unit</td><td>Price</td></tr>" +
        '<tr><td colspan="2">Total</td><td>5</td></tr></table>',
    );
    if (!block || !("Table" in block)) throw new Error("expected a table");
    const rows = block.Table.rows.map((row) => row.cells.map((cell) => text(cell.content)));
    expect(rows).toEqual([
      ["Item", "Unit", "Price"],
      ["Total", "", "5"],
    ]);
  });

  test("a cell merged across rows keeps the cells of the rows below it in place", () => {
    const [block] = pasted(
      "<table><tr><th>Day</th><th>Time</th><th>Room</th></tr>" +
        '<tr><td rowspan="2">Monday</td><td>09:00</td><td>A</td></tr>' +
        "<tr><td>14:00</td><td>B</td></tr>" +
        '<tr><td>Tuesday</td><td colspan="2" rowspan="2">all day</td></tr>' +
        "<tr><td>Wednesday</td></tr></table>",
    );
    if (!block || !("Table" in block)) throw new Error("expected a table");
    const rows = block.Table.rows.map((row) => row.cells.map((cell) => text(cell.content)));
    expect(rows).toEqual([
      ["Day", "Time", "Room"],
      ["Monday", "09:00", "A"],
      ["", "14:00", "B"],
      ["Tuesday", "all day", ""],
      ["Wednesday", "", ""],
    ]);
  });

  test("a list written straight inside a list is the previous item's sub-list", () => {
    const [block] = pasted("<ul><li>a</li><ul><li>b</li></ul><li>c</li></ul>");
    if (!block || !("List" in block)) throw new Error("expected a list");
    expect(block.List.items.map((item) => text(item.content))).toEqual(["a", "c"]);
    expect(block.List.items[0]!.child!.items.map((item) => text(item.content))).toEqual(["b"]);
  });

  test("preformatted text keeps its lines", () => {
    const blocks = paragraphs(pasted("<pre>let a = 1;\nlet b = 2;</pre>"));
    expect(blocks.map((line) => line.map((run) => run.text).join(""))).toEqual(["let a = 1;", "let b = 2;"]);
  });

  test("a style that names no weight keeps the bold it sits in", () => {
    const [line] = paragraphs(pasted('<p><b>bold <span style="font-weight:inherit">still</span></b></p>'));
    expect(line!.every((run) => run.bold)).toBe(true);
  });

  test("a trailing space leaves no empty mark behind", () => {
    const { editor } = harness("");
    const markup = pastedHtml(editor.ownerDocument, '<p>a<a href="https://example.com"> </a></p><p>b</p>');
    expect(markup).toBe("<p>a</p><p>b</p>");
  });

  test("a paste into a list item or a cell arrives as one line", () => {
    const { editor } = harness("");
    const markup = pastedHtml(editor.ownerDocument, "<p>one</p><ul><li>two</li></ul>", true);
    expect(markup).toBe("one two");
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

describe("what a paste carries", () => {
  const png = new File(["x"], "image.png", { type: "image/png" });

  function transfer(flavours: Record<string, string>, files: File[] = []): DataTransfer {
    return {
      items: [],
      files,
      getData: (type: string) => flavours[type] ?? "",
    } as unknown as DataTransfer;
  }

  test("a spreadsheet's cells arrive as a table, not as the picture of them beside it", () => {
    // ONLYOFFICE's Copy of A1:B2, trimmed: the HTML table, the tab-separated text and a PNG
    // rendering of the same cells, all on the clipboard at once.
    const { editor } = harness("");
    const content = pasteContent(
      editor.ownerDocument,
      transfer(
        {
          "text/html":
            '<table class="xslData;XLSY;v2"><tbody><tr><td><span>Row 1 column 1</span></td>' +
            "<td><span>Row 1 column 2</span></td></tr></tbody></table>",
          "text/plain": "Row 1 column 1\tRow 1 column 2",
        },
        [png],
      ),
      false,
    );
    expect("html" in content && content.html).toContain("<table");
  });

  test("a browser's Copy Image arrives as the picture", () => {
    // The HTML beside it names the picture by URL and holds no text.
    const { editor } = harness("");
    const content = pasteContent(
      editor.ownerDocument,
      transfer({ "text/html": '<meta charset="utf-8"><img src="https://example.com/a.png">' }, [png]),
      false,
    );
    expect(content).toEqual({ images: [png] });
  });

  test("text with no formatting arrives as text", () => {
    const { editor } = harness("");
    const content = pasteContent(editor.ownerDocument, transfer({ "text/plain": "plain" }), false);
    expect(content).toEqual({ text: "plain" });
  });
});
