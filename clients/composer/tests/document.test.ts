import { describe, expect, test } from "bun:test";

import { documentBlocks, tableRows } from "../src/document";
import type { Block, InlineContent, InlineImage, TextRun } from "../src/types";
import { harness } from "./support";

/// The character `contenteditable` stores wherever a typed space would otherwise collapse. Written
/// as an escape so it survives review: the literal is invisible in a diff, which is exactly how a
/// wrong one gets in.
const NBSP = "\u00A0";

function blocks(body: string): Block[] {
  return documentBlocks(harness(body).editor);
}

function runsOf(content: InlineContent[]): TextRun[] {
  return content.filter((item): item is { Text: TextRun } => "Text" in item).map((item) => item.Text);
}

function imagesOf(content: InlineContent[]): InlineImage[] {
  return content
    .filter((item): item is { Image: InlineImage } => "Image" in item)
    .map((item) => item.Image);
}

/// The one image `body` is expected to produce, so a test that silently emits none or two fails
/// here rather than passing on an assertion against nothing.
function onlyImage(body: string): InlineImage {
  const images = imagesOf(paragraphs(body)[0] ?? []);
  if (images.length !== 1) throw new Error(`expected one image, got ${images.length}`);
  return images[0]!;
}

/// The inline content of each `Paragraph`, in document order.
function paragraphs(body: string): InlineContent[][] {
  return blocks(body)
    .filter((block): block is { Paragraph: { content: InlineContent[] } } => "Paragraph" in block)
    .map((block) => block.Paragraph.content);
}

/// Each paragraph's text, its runs concatenated; the shape most of these assertions want.
function texts(body: string): string[] {
  return paragraphs(body).map((content) =>
    runsOf(content)
      .map((run) => run.text)
      .join(""),
  );
}

function firstTable(body: string): Element {
  const table = harness(body).editor.querySelector("table");
  if (!table) throw new Error("no table in fixture");
  return table as unknown as Element;
}

describe("non-breaking spaces are normalized out of body text", () => {
  test("a typed space stored as U+00A0 reaches Rust as a plain space", () => {
    expect(texts(`<p>a${NBSP}b</p>`)).toEqual(["a b"]);
  });

  test("the entity spelling normalizes too", () => {
    // How pasted mail arrives; the parser resolves it to the same code point.
    expect(texts("<p>a&nbsp;b</p>")).toEqual(["a b"]);
  });

  test("every occurrence goes, not only the first", () => {
    expect(texts(`<p>a${NBSP}b${NBSP}c</p>`)).toEqual(["a b c"]);
  });

  test("a run of them keeps its width", () => {
    // Indenting by pressing space arrives as a run of NBSPs. One space each, so the indent the
    // user can see survives into the text/plain part instead of collapsing to one.
    expect(texts(`<p>${NBSP}${NBSP}${NBSP}x</p>`)).toEqual(["   x"]);
  });

  test("it reaches text nested under marks", () => {
    expect(texts(`<p><b>a${NBSP}b</b></p>`)).toEqual(["a b"]);
  });

  test("it reaches list items and table cells, not just paragraphs", () => {
    const list = blocks(`<ul><li>a${NBSP}b</li></ul>`)[0] as {
      List: { items: { content: InlineContent[] }[] };
    };
    expect(runsOf(list.List.items[0]!.content)[0]!.text).toBe("a b");

    const table = blocks(`<table><tbody><tr><td>a${NBSP}b</td></tr></tbody></table>`)[0] as {
      Table: { rows: { cells: { content: InlineContent[] }[] }[] };
    };
    expect(runsOf(table.Table.rows[0]!.cells[0]!.content)[0]!.text).toBe("a b");
  });

  test("a quoted original keeps its own, because it rides out as raw HTML", () => {
    // The boundary worth knowing: `normalizeText` walks text nodes, and a quote is emitted
    // verbatim so the original author's message is not rewritten on its way back out. It comes
    // back as the entity because HTML serialization escapes U+00A0 in a text node.
    const quoted = blocks(
      `<div class="allodia-quote"><div class="aq-body">a${NBSP}b</div></div>`,
    )[0] as { Quote: { body_html: string } };
    expect(quoted.Quote.body_html).toBe("a&nbsp;b");
  });
});

describe("an empty document still has a paragraph", () => {
  test("an empty editor", () => {
    expect(blocks("")).toEqual([{ Paragraph: { content: [] } }]);
  });

  test("a lone placeholder <br>", () => {
    expect(blocks("<p><br></p>")).toEqual([{ Paragraph: { content: [] } }]);
  });
});

describe("soft breaks become real paragraphs", () => {
  test("a <br> splits one line in two", () => {
    expect(texts("<div>a<br>b</div>")).toEqual(["a", "b"]);
  });

  test("a single trailing <br> is a placeholder, not an empty line", () => {
    expect(texts("<div>a<br></div>")).toEqual(["a"]);
  });

  test("a second trailing <br> is a deliberate blank line", () => {
    expect(texts("<div>a<br><br></div>")).toEqual(["a", ""]);
  });

  test("a top-level <br> ends the line", () => {
    expect(texts("a<br>b")).toEqual(["a", "b"]);
  });

  test("a <br> nested inside a mark is dropped and the two sides join", () => {
    // Known shortfall, pinned so a change to it is deliberate: `leafParagraphs` splits on the
    // leaf's OWN children, so a break inside a <span> is not a split point and contributes no
    // content. Pasted markup can carry one.
    expect(texts("<div><span>a<br>b</span></div>")).toEqual(["ab"]);
  });
});

describe("inline images", () => {
  test("an image carries its attachment id, alt text and width", () => {
    expect(paragraphs('<p><img data-attachment-id="att1" alt="a cat" width="320"></p>')[0]).toEqual([
      { Image: { attachment_id: "att1", alt_text: "a cat", width_px: 320 } },
    ]);
  });

  test("a width set through the style attribute counts too", () => {
    // Resizing in the editor writes a style, not the attribute.
    expect(onlyImage('<p><img data-attachment-id="att1" style="width: 240px"></p>').width_px).toBe(
      240,
    );
  });

  test("the width attribute wins over the style, rather than the two racing", () => {
    expect(
      onlyImage('<p><img data-attachment-id="att1" width="320" style="width: 240px"></p>').width_px,
    ).toBe(320);
  });

  test("no width is null rather than absent, and a degenerate one is null too", () => {
    // Unlike a TextRun's optional marks, `width_px` is emitted: Rust reads it as "the user did not
    // resize this", and 0 would be a real width that renders nothing.
    const none = onlyImage('<p><img data-attachment-id="att1"></p>');
    expect(none.width_px).toBeNull();
    expect(Object.keys(none)).toContain("width_px");

    expect(onlyImage('<p><img data-attachment-id="att1" width="0"></p>').width_px).toBeNull();
    expect(onlyImage('<p><img data-attachment-id="att1" width="wide"></p>').width_px).toBeNull();
  });

  test("missing alt text is an empty string, never absent", () => {
    expect(onlyImage('<p><img data-attachment-id="att1"></p>').alt_text).toBe("");
  });

  test("an image with no attachment id is dropped, and the text around it survives", () => {
    // Nothing in the draft's attachment list backs it, so emitting it would reference a blob that
    // does not exist.
    expect(texts('<p>before<img src="https://tracker.example/x.png">after</p>')).toEqual([
      "beforeafter",
    ]);
    expect(paragraphs('<p>before<img src="https://tracker.example/x.png">after</p>')[0]).toHaveLength(
      2,
    );
  });
});

describe("a text run omits the marks it does not carry", () => {
  // The discard probe diffs this document on every close, so an absent key; not a null; is what
  // keeps an untouched draft byte-identical to its seed.
  test("a plain run has only the four required keys", () => {
    const [run] = runsOf(paragraphs("<p>hi</p>")[0]!);
    expect(Object.keys(run!).sort()).toEqual(["bold", "italic", "text", "underline"]);
  });

  test("a coloured run adds exactly one key", () => {
    const [run] = runsOf(paragraphs('<p><span data-color="#ff0000">hi</span></p>')[0]!);
    expect(Object.keys(run!).sort()).toEqual(["bold", "color", "italic", "text", "underline"]);
    expect(run!.color).toBe("#ff0000");
  });

  test("a sized and highlighted run adds only those", () => {
    const [run] = runsOf(
      paragraphs('<p><span data-size="Large" data-highlight="#ffff00">hi</span></p>')[0]!,
    );
    expect(Object.keys(run!).sort()).toEqual([
      "bold",
      "font_size",
      "highlight",
      "italic",
      "text",
      "underline",
    ]);
  });
});

describe("tableRows reads this table's rows only", () => {
  test("rows under thead, tbody and tfoot all count, in document order", () => {
    const table = firstTable(
      "<table>" +
        "<thead><tr><th>h</th></tr></thead>" +
        "<tbody><tr><td>a</td></tr><tr><td>b</td></tr></tbody>" +
        "<tfoot><tr><td>f</td></tr></tfoot>" +
        "</table>",
    );
    expect(tableRows(table).map((row) => row.textContent)).toEqual(["h", "a", "b", "f"]);
  });

  test("a bare <tr> child, with no section wrapper, counts", () => {
    expect(tableRows(firstTable("<table><tr><td>a</td></tr></table>"))).toHaveLength(1);
  });

  test("a nested table's rows are not hoisted into the outer one", () => {
    // A descendant query would corrupt the outer table's shape, and Rust rejects a ragged table,
    // a failed send rather than a broken render.
    const outer = firstTable(
      "<table><tbody><tr><td>" +
        "<table><tbody><tr><td>inner</td></tr><tr><td>inner2</td></tr></tbody></table>" +
        "</td></tr></tbody></table>",
    );
    expect(tableRows(outer)).toHaveLength(1);
  });

  test("a non-row child is ignored rather than emitted as a row", () => {
    expect(tableRows(firstTable("<table><caption>c</caption><tr><td>a</td></tr></table>"))).toHaveLength(
      1,
    );
  });
});

describe("block structure", () => {
  test("bare inline text at the editor root becomes a paragraph", () => {
    expect(texts("hello")).toEqual(["hello"]);
  });

  test("adjacent inline nodes accumulate into one paragraph, not one each", () => {
    expect(paragraphs("<b>a</b><i>b</i>c")).toHaveLength(1);
    expect(texts("<b>a</b><i>b</i>c")).toEqual(["abc"]);
  });

  test("a wrapper div is recursed into, so the list inside stays a list", () => {
    expect(blocks("<div><ul><li>x</li></ul></div>")).toEqual([
      {
        List: {
          kind: "Bullet",
          items: [
            { content: [{ Text: { text: "x", bold: false, italic: false, underline: false } }], child: null },
          ],
        },
      },
    ]);
  });

  test("a quoted original is one Quote block, never flattened into paragraphs", () => {
    const result = blocks(
      '<p>my reply</p>' +
        '<div class="allodia-quote" data-quote-style="LineAndHeader" data-quote-line="On Monday, Alice wrote:" data-quote-plain="original">' +
        '<div class="aq-attr">On Monday, Alice wrote:</div>' +
        '<div class="aq-body"><p>original</p></div>' +
        "</div>",
    );
    expect(result.map((block) => Object.keys(block)[0])).toEqual(["Paragraph", "Quote"]);
    const quote = result[1] as { Quote: { style: string; body_html: string; body_plain: string } };
    expect(quote.Quote.style).toBe("LineAndHeader");
    expect(quote.Quote.body_html).toBe("<p>original</p>");
    expect(quote.Quote.body_plain).toBe("original");
  });

  test("the signature is one Signature block, keeping its markup intact", () => {
    const result = blocks(
      "<p>my message</p>" +
        '<div class="allodia-signature" data-signature-plain="Alice"><p><b>Alice</b></p></div>',
    );
    expect(result.map((block) => Object.keys(block)[0])).toEqual(["Paragraph", "Signature"]);
    const signature = result[1] as { Signature: { body_html: string; body_plain: string } };
    expect(signature.Signature.body_html).toBe("<p><b>Alice</b></p>");
    expect(signature.Signature.body_plain).toBe("Alice");
  });

  test("a signature inside the quoted original stays inside it", () => {
    // Replying to our own mail nests one quote-side signature; hoisting it would emit the original
    // author's sign-off as this message's.
    const result = blocks(
      '<div class="allodia-quote"><div class="aq-body">' +
        '<div class="allodia-signature"><p>Alice</p></div>' +
        "</div></div>",
    );
    expect(result.map((block) => Object.keys(block)[0])).toEqual(["Quote"]);
  });
});
