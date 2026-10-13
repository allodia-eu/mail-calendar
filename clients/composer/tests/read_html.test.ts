// A stored body reopened in the composer: what this app rendered reads back as the document it came
// from, and what another client wrote reads as closely as the schema can hold it.

import { describe, expect, test } from "bun:test";

import { Attachments } from "../src/attachments";
import { documentBlocks } from "../src/document";
import { plainText } from "../src/plain_text";
import { setComposerBody } from "../src/seeds";
import type { Block, ComposerDocument, DraftAttachment, InlineContent, TextRun } from "../src/types";
import rendered from "./fixtures/rendered.json";
import { harness } from "./support";

interface Entry {
  name: string;
  document: ComposerDocument;
  html: string;
}

/// The body as the core hands it over: every `cid:` picture already turned into the `data:` URI of
/// its part, which is what the reading path does before a host sees a message.
function asSeeded(entry: Entry): string {
  return entry.document.attachments.reduce((html, attachment: DraftAttachment) => {
    const disposition = attachment.disposition;
    if (typeof disposition === "string" || !attachment.data_url) return html;
    return html.split(`cid:${disposition.Inline.cid}`).join(attachment.data_url);
  }, entry.html);
}

/// Opens `html` in a fresh composer and reads it back the way a save does.
function reopen(html: string): { blocks: Block[]; attachments: Attachments; editor: HTMLElement } {
  const { editor } = harness("");
  const attachments = new Attachments();
  setComposerBody(editor, attachments, { html });
  return { blocks: documentBlocks(editor), attachments, editor };
}

function run(text: string, marks: Partial<TextRun> = {}): InlineContent {
  return { Text: { text, bold: false, italic: false, underline: false, ...marks } };
}

describe("a body this app rendered", () => {
  for (const entry of rendered as unknown as Entry[]) {
    test(`reads back as its document: ${entry.name}`, () => {
      const { blocks, attachments } = reopen(asSeeded(entry));
      expect(blocks).toEqual(entry.document.blocks);
      for (const attachment of entry.document.attachments) {
        expect(attachments.dataUrlOf(attachment.id)).toBe(attachment.data_url ?? "");
      }
    });
  }
});

describe("a body another client wrote", () => {
  test("its indentation is not read as text", () => {
    const { blocks } = reopen(`
      <div>
        <p>
          First line
        </p>
        <p>Second   line</p>
      </div>`);
    expect(blocks).toEqual([
      { Paragraph: { content: [run("First line")], spaced: true } },
      { Paragraph: { content: [run("Second line")], spaced: true } },
    ]);
  });

  test("a paragraph is set apart and a line is not, however each client spells a line", () => {
    const { blocks } = reopen(
      '<p>Thunderbird</p><div>Gmail</div><p class="MsoNormal">Outlook</p>' +
        '<p style="margin: 0">Flush</p><p>One<br>Two</p>',
    );
    expect(blocks).toEqual([
      { Paragraph: { content: [run("Thunderbird")], spaced: true } },
      { Paragraph: { content: [run("Gmail")] } },
      { Paragraph: { content: [run("Outlook")] } },
      { Paragraph: { content: [run("Flush")] } },
      // A break inside a paragraph breaks the line: only the paragraph's last line is set apart.
      { Paragraph: { content: [run("One")] } },
      { Paragraph: { content: [run("Two")], spaced: true } },
    ]);
  });

  test("a heading is a line's style, not a mark on its words", () => {
    const { blocks } = reopen("<h1>Agenda</h1><h3>Items</h3><p>One</p>");
    expect(blocks).toEqual([
      { Paragraph: { content: [run("Agenda")], heading: "H1" } },
      { Paragraph: { content: [run("Items")], heading: "H3" } },
      { Paragraph: { content: [run("One")], spaced: true } },
    ]);
  });

  test("a heading where no line can be one reads as the bold, larger run it looks like", () => {
    const { blocks } = reopen("<ul><li><h1>Agenda</h1></li></ul>");
    expect(blocks).toEqual([
      { List: { kind: "Bullet", items: [{ content: [run("Agenda", { bold: true, font_size: 24 })], child: null }] } },
    ]);
  });

  test("marks given as inline style are read without a window to compute them, sizes in points too", () => {
    const { blocks } = reopen(
      '<p><span style="font-weight: bold">b</span><span style="font-style: italic">i</span>' +
        '<span style="text-decoration: underline">u</span><span style="font-size: 11pt">s</span></p>',
    );
    expect(blocks).toEqual([
      {
        Paragraph: {
          content: [
            run("b", { bold: true }),
            run("i", { italic: true }),
            run("u", { underline: true }),
            run("s", { font_size: 15 }),
          ],
          spaced: true,
        },
      },
    ]);
  });

  test("a line's own style reaches its words", () => {
    const { blocks } = reopen('<div style="color: #336699"><p>Blue</p></div>');
    expect(blocks).toEqual([{ Paragraph: { content: [run("Blue", { color: "#336699" })], spaced: true } }]);
  });

  test("a Gmail quote becomes the quote region, its attribution the quote's line", () => {
    const { blocks } = reopen(
      '<div>Sounds good.</div><div class="gmail_quote"><div class="gmail_attr">On Mon, Bob wrote:<br></div>' +
        '<blockquote class="gmail_quote" style="margin:0 0 0 .8ex">Lunch?<br>Bob</blockquote></div>',
    );
    expect(blocks).toEqual([
      { Paragraph: { content: [run("Sounds good.")] } },
      {
        Quote: {
          style: "Indented",
          attribution: { line: "On Mon, Bob wrote:", headers: [] },
          body_html: "Lunch?<br>Bob",
          body_plain: "Lunch?\nBob",
        },
      },
    ]);
  });

  test("a preformatted body keeps its lines", () => {
    const { blocks } = reopen("<pre>one\ntwo</pre>");
    expect(blocks).toEqual([
      { Paragraph: { content: [run("one")] } },
      { Paragraph: { content: [run("two")] } },
    ]);
  });

  test("a picture it links from the web is not kept, because the composer cannot send one", () => {
    const { blocks, editor } = reopen('<p>Logo <img src="https://example.test/logo.png" alt="logo"></p>');
    expect(blocks).toEqual([{ Paragraph: { content: [run("Logo")], spaced: true } }]);
    expect(editor.querySelector("img")).toBeNull();
  });

  test("a picture it carries is named for the type its bytes declare", () => {
    const png = "data:image/png;base64,iVBORw0KGgo=";
    const { attachments } = reopen(`<p><img src="${png}" alt="dot"></p>`);
    expect(attachments.list().map(({ file_name, media_type }) => ({ file_name, media_type }))).toEqual([
      { file_name: "image-0.png", media_type: "image/png" },
    ]);
  });

  test("nothing it carries outside the body is read as words", () => {
    const { blocks } = reopen(
      "<html><head><title>Draft</title><style>p { color: red }</style></head><body><p>Hi</p></body></html>",
    );
    expect(blocks).toEqual([{ Paragraph: { content: [run("Hi")], spaced: true } }]);
  });
});

describe("a body with no HTML", () => {
  test("opens as its text, a line per line", () => {
    const { editor } = harness("");
    setComposerBody(editor, new Attachments(), { html: "", text: "One\n\nTwo" });
    expect(documentBlocks(editor)).toEqual([
      { Paragraph: { content: [run("One")] } },
      { Paragraph: { content: [] } },
      { Paragraph: { content: [run("Two")] } },
    ]);
  });
});

describe("the plain text of a quote or a signature", () => {
  test("keeps its lines apart", () => {
    const { editor } = harness("<div id='x'><p>Ada</p><div>Engines<br>London</div></div>");
    expect(plainText(editor.querySelector("#x")!)).toBe("Ada\nEngines\nLondon");
  });
});
