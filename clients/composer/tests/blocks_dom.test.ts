// Building the editor's DOM from a document, held to `documentBlocks` reading it back unchanged.

import { describe, expect, test } from "bun:test";

import { Attachments } from "../src/attachments";
import { setBlocks } from "../src/blocks_dom";
import { documentBlocks } from "../src/document";
import type { Block, ComposerDocument } from "../src/types";
import rendered from "./fixtures/rendered.json";
import { harness } from "./support";

/// The manifest a document's pictures are drawn from, holding each under the id the document
/// names. Captured pictures are minted `captured-0`, `captured-1`, … in order, which is what the
/// fixture's ids are.
function manifestFor(document: ComposerDocument): Attachments {
  const attachments = new Attachments();
  for (const attachment of document.attachments) {
    if (!attachment.data_url) continue;
    attachments.addCapturedImage({
      data_url: attachment.data_url,
      file_name: attachment.file_name,
      media_type: attachment.media_type,
    });
  }
  return attachments;
}

describe("a document built into the editor", () => {
  for (const entry of rendered as unknown as { name: string; document: ComposerDocument }[]) {
    test(`reads back unchanged: ${entry.name}`, () => {
      const { editor } = harness("");
      setBlocks(editor, entry.document.blocks, manifestFor(entry.document));
      expect(documentBlocks(editor)).toEqual(entry.document.blocks);
    });
  }

  test("that opens on its signature gets a line to write on above it", () => {
    const { editor } = harness("");
    const blocks: Block[] = [{ Signature: { body_html: "<p>Ada</p>", body_plain: "Ada" } }];
    setBlocks(editor, blocks, new Attachments());
    expect(editor.firstElementChild?.tagName).toBe("P");
    expect(editor.children[1]?.classList.contains("allodia-signature")).toBe(true);
  });

  test("leaves out a picture whose bytes it was not given, rather than drawing it broken", () => {
    const { editor } = harness("");
    const blocks: Block[] = [
      { Paragraph: { content: [{ Image: { attachment_id: "missing", alt_text: "", width_px: null } }] } },
    ];
    setBlocks(editor, blocks, new Attachments());
    expect(editor.querySelector("img")).toBeNull();
    expect(documentBlocks(editor)).toEqual([{ Paragraph: { content: [] } }]);
  });

  test("marks a run the way the toolbar does, so it is edited like a typed one", () => {
    const { editor } = harness("");
    const blocks: Block[] = [
      {
        Paragraph: {
          content: [{ Text: { text: "big", bold: false, italic: false, underline: false, font_size: "Large" } }],
        },
      },
    ];
    setBlocks(editor, blocks, new Attachments());
    const span = editor.querySelector("span")!;
    expect(span.dataset.size).toBe("Large");
    expect(span.style.fontSize).toBe("18px");
  });
});
