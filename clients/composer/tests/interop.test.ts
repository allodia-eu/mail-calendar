// A draft Thunderbird or Outlook stored reopens here as they showed it.
//
// Each entry is a real draft one of them saved (`fixtures/interop/*.eml`), as the core hands it to
// the composer: through the engine's MIME reading and the core's sanitiser, its pictures inlined
// (`crates/mailcal-app/src/draft_interop_tests.rs` writes `fixtures/interop.json`). What each test
// expects is what that client drew when the draft was reopened in it. Outlook stored its reply and
// forwards as plain text, because the message it answered was; those three carry no HTML here.

import { describe, expect, test } from "bun:test";

import { Attachments } from "../src/attachments";
import { documentBlocks } from "../src/document";
import { setComposerBody } from "../src/seeds";
import type { Block, InlineContent, TextRun } from "../src/types";
import interop from "./fixtures/interop.json";
import { harness } from "./support";

interface Entry {
  name: string;
  html: string;
  text: string;
}

function reopen(name: string): Block[] {
  const entry = (interop as Entry[]).find((candidate) => candidate.name === name);
  if (!entry) throw new Error(`no interop fixture named ${name}`);
  const { editor } = harness("");
  setComposerBody(editor, new Attachments(), { html: entry.html });
  return documentBlocks(editor);
}

function run(text: string, marks: Partial<TextRun> = {}): InlineContent {
  return { Text: { text, bold: false, italic: false, underline: false, ...marks } };
}

/// A line, as Outlook writes every one and as text with no element around it is.
function line(...content: InlineContent[]): Block {
  return { Paragraph: { content } };
}

/// A paragraph, set apart from the next, as Thunderbird writes every `<p>`.
function para(...content: InlineContent[]): Block {
  return { Paragraph: { content, spaced: true } };
}

const BLANK: Block = { Paragraph: { content: [] } };

/// The nested list both clients wrote: the numbered list sits directly inside the bulleted one,
/// and belongs to its first item.
const NESTED_LIST: Block = {
  List: {
    kind: "Bullet",
    items: [
      {
        content: [run("Bullet item 1")],
        child: {
          kind: "Ordered",
          items: [
            { content: [run("Numbered list nested inside it")], child: null },
            { content: [run("Second numbered list item")], child: null },
            { content: [run("Third one")], child: null },
          ],
        },
      },
      { content: [run("Bullet item 2")], child: null },
      { content: [run("Bullet item 3")], child: null },
    ],
  },
};

function table(): Block {
  const cells = (...texts: string[]) => ({ cells: texts.map((text) => ({ content: [run(text)] })) });
  return { Table: { rows: [cells("Heading 1", "Heading 2"), cells("Row 2, first cell", "Row 2, second cell")] } };
}

function picture(blocks: Block[], width: number, alt: string): Block {
  const found = blocks.find(
    (block) => "Paragraph" in block && block.Paragraph.content.some((inline) => "Image" in inline),
  );
  expect(found).toBeDefined();
  const image = (found as { Paragraph: { content: InlineContent[] } }).Paragraph.content[0]!;
  expect(image).toEqual({ Image: { attachment_id: expect.any(String), alt_text: alt, width_px: width } });
  return found!;
}

describe("a draft Thunderbird stored", () => {
  test("a new message keeps every format Thunderbird drew", () => {
    const blocks = reopen("thunderbird-new-message");
    const signature = blocks.at(-1)!;
    expect(blocks).toEqual([
      para(run("Hey there,")),
      para(
        run("This is "),
        run("bold", { bold: true }),
        run(" text, and "),
        run("underlined", { underline: true }),
        run(" is quite nice too. What about "),
        run("italic?", { italic: true }),
      ),
      para(
        run("This is regular size. "),
        run("Larger now. ", { font_size: 32 }),
        run("Slightly smaller.", { font_size: 24 }),
        run(" Smaller. ", { font_size: 18 }),
        run("Regular again. "),
        run("Very small. ", { font_size: 13 }),
        run("Tiny.", { font_size: 10 }),
      ),
      para(run("Wow and what about emoji? 🙂😉😛")),
      { Paragraph: { content: [run("Heading 1")], heading: "H1" } },
      para(run("That's a big one. "), run("Here's a link", { link: "https://allodia.eu" }), run(" by the way.")),
      para(
        run("And also "),
        run("different ", { color: "#ff0000" }),
        run("colors ", { color: "#00ff80" }),
        run("are ", { color: "#ff00ff" }),
        run("nice ", { color: "#0080ff" }),
        run("to play with.", { color: "#000000" }),
      ),
      para(
        run("W", { color: "#000000" }),
        run("hat about a different font? Famous Comic Sans!", { color: "#000000", font: "Comic Sans MS" }),
      ),
      line(run("Here's a list.")),
      NESTED_LIST,
      { Paragraph: { content: [run("Indentation also works!")], spaced: true, indent_px: 40 } },
      { Paragraph: { content: [run("Indented even further.")], spaced: true, indent_px: 80 } },
      para(run("Now back to normal.")),
      { Paragraph: { content: [run("Even centered text works!")], spaced: true, align: "Center" } },
      para(run("Here's an inline image:")),
      picture(blocks, 373, "Alternate text here"),
      para(run("And a small table:")),
      table(),
      { Rule: {} },
      line(run("Wow that's a horizontal line right above me.")),
      signature,
    ]);
    expect(signature).toEqual({
      Signature: {
        body_html: expect.stringContaining("<b>This is my bold signature, pretty cool"),
        body_plain: expect.stringMatching(/^--\nKind regards, Alice This is my bold signature/),
      },
    });
  });

  test("a reply keeps its quote, under the attribution Thunderbird wrote, and its signature", () => {
    const blocks = reopen("thunderbird-reply");
    expect(blocks).toEqual([
      para(run("This is a draft reply from Thunderbird to the baseline message.")),
      {
        Quote: {
          style: "Indented",
          attribution: { line: "Op 7-1-2026 om 11:00 schreef Newsletter:", headers: [] },
          body_html: expect.stringContaining('<pre class="moz-quote-pre">Second of two messages'),
          body_plain: expect.stringContaining("Second of two messages"),
        },
      },
      {
        Signature: {
          body_html: expect.stringContaining("<b>This is my bold signature, pretty cool right?</b>"),
          body_plain: expect.stringMatching(/^--\nKind regards,/),
        },
      },
    ]);
  });

  test.each(["thunderbird-forward", "thunderbird-forward-with-file"])(
    "%s keeps the forwarded message under the headers Thunderbird listed",
    (name) => {
      const [text, quote, ...rest] = reopen(name);
      expect(text).toEqual(para(run(expect.stringMatching(/^This is (a|the) forward/) as unknown as string)));
      expect(rest).toEqual([]);
      const value = (quote as Extract<Block, { Quote: unknown }>).Quote;
      expect(value.style).toBe("LineAndHeader");
      expect(value.attribution.headers.map((header) => header.label)).toEqual(["Onderwerp", "Datum", "Van", "Aan"]);
      expect(value.attribution.headers[2]!.value).toMatch(/^[A-Z][a-z]+ (Tester )?<[a-z]+@(test\.local|example\.com)>$/);
      expect(value.body_html).not.toContain("Doorgestuurd bericht");
      expect(value.body_plain.trim()).toMatch(/^(Baseline plaintext message|Body part\.)/);
    },
  );
});

describe("a draft Outlook stored", () => {
  test("a new message keeps every format Outlook drew, blank lines included", () => {
    const blocks = reopen("outlook-new-message");
    const signature = blocks.at(-3)!;
    expect(blocks).toEqual([
      line(run("Hey there,")),
      BLANK,
      line(
        run("This is "),
        run("bold", { bold: true }),
        run(" text, and "),
        run("underlined", { underline: true }),
        run(" is quite nice too. What about "),
        run("italic?", { italic: true }),
      ),
      BLANK,
      line(
        run("This is regular size. "),
        run("Larger now. ", { font_size: 32 }),
        run("Slightly smaller. ", { font_size: 27 }),
        run("Smaller. ", { font_size: 21 }),
        run("Regular again. "),
        run("Very small. ", { font_size: 13 }),
        run("Tiny.", { font_size: 11 }),
      ),
      BLANK,
      line(run("Wow and what about emoji? "), run("🙂😉😛", { font: "Segoe UI Emoji, sans-serif" })),
      { Paragraph: { content: [run("Heading 1")], heading: "H1" } },
      line(run("That’s a big one. "), run("Here’s a link", { link: "https://allodia.eu/" }), run(" by the way.")),
      BLANK,
      line(
        run("And also "),
        run("different ", { color: "#ee0000" }),
        run("colors ", { color: "#92d050" }),
        run("are ", { color: "#d86ecc" }),
        run("nice ", { color: "#4e95d9" }),
        run("to play with."),
      ),
      BLANK,
      line(run("What about a different font? Famous Comic Sans!", { font: "Comic Sans MS" })),
      BLANK,
      line(run("Here’s a list.")),
      BLANK,
      NESTED_LIST,
      BLANK,
      // Outlook's first indent is a quarter inch, its second 35.4pt: kept as it drew them.
      { Paragraph: { content: [run("Indentation also works!")], indent_px: 24 } },
      { Paragraph: { content: [run("Indented even further.")], indent_px: 47 } },
      BLANK,
      line(run("Now back to normal.")),
      BLANK,
      { Paragraph: { content: [run("Even centered text works!")], align: "Center" } },
      BLANK,
      line(run("Here’s an inline image:")),
      BLANK,
      picture(blocks, 407, ""),
      BLANK,
      line(run("And a small table:")),
      BLANK,
      table(),
      BLANK,
      { Rule: {} },
      BLANK,
      line(run("Wow that’s a horizontal line right above me.")),
      BLANK,
      signature,
      BLANK,
      BLANK,
    ]);
    expect(signature).toEqual({
      Signature: {
        body_html: expect.stringContaining('name="_MailAutoSig"'),
        body_plain: "Kind regards,\nAlice\nThis is my bold signature, pretty cool right?",
      },
    });
  });

  test.each(["outlook-reply", "outlook-forward", "outlook-forward-with-file"])(
    "%s is plain text, as the message it answered was",
    (name) => {
      const entry = (interop as Entry[]).find((candidate) => candidate.name === name)!;
      expect(entry.html).toBe("");
      expect(entry.text).toContain("-----Oorspronkelijk bericht-----");
    },
  );

  test.each(["outlook-reply", "outlook-forward", "outlook-forward-with-file"])(
    "%s reopens line for line, blank lines included",
    (name) => {
      const entry = (interop as Entry[]).find((candidate) => candidate.name === name)!;
      const { editor } = harness("");
      setComposerBody(editor, new Attachments(), { html: entry.html, text: entry.text });
      const lines = documentBlocks(editor).map((block) =>
        "Paragraph" in block
          ? block.Paragraph.content.map((inline) => ("Text" in inline ? inline.Text.text : "")).join("")
          : "",
      );
      expect(lines).toEqual(entry.text.split("\r\n"));
    },
  );
});
