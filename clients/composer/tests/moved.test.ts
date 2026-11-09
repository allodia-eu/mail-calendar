// A composer moved to another view of it opens holding exactly the document the one it left wrote.
//
// The inline composer popped out into a window hands its `composerDocument()` to the window's
// editor, which seeds it through `setComposerDocument`. Anything that changed on the way would be
// saved over the user's draft by the window's next save (`docs/reading-window.md`).

import { describe, expect, test } from "bun:test";

import { Attachments } from "../src/attachments";
import { setBlocks } from "../src/blocks_dom";
import { documentBlocks, referencedAttachmentIds } from "../src/document";
import { setComposerDocument } from "../src/seeds";
import type { Block, ComposerDocument } from "../src/types";
import { harness } from "./support";

const PIXEL = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==";

/// What `composerDocument()` answers for an editor: its blocks and the manifest they reference.
function documentOf(editor: HTMLElement, attachments: Attachments): ComposerDocument {
  const blocks = documentBlocks(editor);
  return { blocks, attachments: attachments.list(referencedAttachmentIds(blocks)) };
}

function written(): { editor: HTMLElement; attachments: Attachments } {
  const { editor } = harness("");
  const attachments = new Attachments();
  const picture = attachments.addCapturedImage({ data_url: PIXEL, file_name: "pixel.png", media_type: "image/png" });
  const blocks: Block[] = [
    {
      Paragraph: {
        content: [
          { Text: { text: "Half a reply, ", bold: false, italic: false, underline: false } },
          { Text: { text: "with weight", bold: true, italic: false, underline: false } },
        ],
      },
    },
    { Paragraph: { content: [{ Image: { attachment_id: picture, alt_text: "a pixel", width_px: 1 } }] } },
    { Signature: { body_html: "<p>Ada</p>", body_plain: "Ada" } },
    {
      Quote: {
        style: "LineAndHeader",
        attribution: { line: "On Monday, Bob wrote:", headers: [{ label: "From", value: "Bob" }] },
        body_html: "<p>The original</p>",
        body_plain: "The original",
      },
    },
  ];
  setBlocks(editor, blocks, attachments);
  return { editor, attachments };
}

describe("a moved composer", () => {
  test("opens on the document the one it left wrote, picture, signature and quote included", () => {
    const left = written();
    const handed = documentOf(left.editor, left.attachments);

    const { editor } = harness("");
    const attachments = new Attachments();
    setComposerDocument(editor, attachments, JSON.parse(JSON.stringify(handed)));

    expect(documentOf(editor, attachments)).toEqual(handed);
  });

  test("numbers a picture captured after the move past the ones it brought", () => {
    const left = written();
    const handed = documentOf(left.editor, left.attachments);
    const attachments = new Attachments();
    setComposerDocument(harness("").editor, attachments, handed);

    const next = attachments.addCapturedImage({ data_url: PIXEL, file_name: "second.png", media_type: "image/png" });

    expect(handed.attachments.map((attachment) => attachment.id)).toEqual(["captured-0"]);
    expect(next).toBe("captured-1");
  });

  test("that held nothing opens holding nothing, whatever the editor held before", () => {
    const empty = harness("");
    const handed = documentOf(empty.editor, new Attachments());
    const { editor } = harness("<div>left over</div>");
    const attachments = new Attachments();

    setComposerDocument(editor, attachments, handed);

    expect(documentOf(editor, attachments)).toEqual(handed);
    expect(editor.textContent).toBe("");
  });
});
