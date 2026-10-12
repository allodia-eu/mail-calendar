// A draft reopens exactly as it was saved: for documents generated across the whole schema, what
// the editor reads back from the HTML a save and a reopen produce is the document it held when it
// saved. The core generates the fixture and passes each document through its own save and reopen
// paths (`crates/mailcal-app/src/draft_roundtrip_tests.rs`); this suite holds the reader to it.

import { describe, expect, test } from "bun:test";

import { Attachments } from "../src/attachments";
import { setBlocks } from "../src/blocks_dom";
import { documentBlocks, referencedAttachmentIds } from "../src/document";
import { setComposerBody } from "../src/seeds";
import type { Block, ComposerDocument, DraftAttachment } from "../src/types";
import reopened from "./fixtures/reopened.json";
import { harness } from "./support";

interface Entry {
  name: string;
  document: ComposerDocument;
  html: string;
}

/// The manifest the document's pictures are drawn from, each under the id and `Content-ID` the
/// document names.
function manifestFor(document: ComposerDocument): Attachments {
  const attachments = new Attachments();
  for (const attachment of document.attachments) {
    const disposition = attachment.disposition;
    if (!attachment.data_url || typeof disposition === "string") continue;
    attachments.addCapturedImage({
      data_url: attachment.data_url,
      file_name: attachment.file_name,
      media_type: attachment.media_type,
      content_id: disposition.Inline.cid,
    });
  }
  return attachments;
}

/// What the editor emits for `blocks` and `attachments` once it holds them.
function emitted(blocks: Block[], attachments: Attachments): { blocks: Block[]; attachments: DraftAttachment[] } {
  return { blocks, attachments: attachments.list(referencedAttachmentIds(blocks)) };
}

/// `blocks` with each region's plain text left out: the generator does not compute it, the editor
/// derives it from the region's markup.
function withoutPlainText(blocks: Block[]): Block[] {
  return blocks.map((block) => {
    if ("Quote" in block) return { Quote: { ...block.Quote, body_plain: "" } };
    if ("Signature" in block) return { Signature: { ...block.Signature, body_plain: "" } };
    return block;
  });
}

describe("a saved draft reopened", () => {
  for (const entry of reopened as unknown as Entry[]) {
    test(`is the document it was saved from: ${entry.name}`, () => {
      const held = harness("");
      const heldAttachments = manifestFor(entry.document);
      setBlocks(held.editor, entry.document.blocks, heldAttachments);
      const saved = emitted(documentBlocks(held.editor), heldAttachments);
      // The generator writes documents in the editor's own form, so holding one changes nothing.
      expect(withoutPlainText(saved.blocks)).toEqual(entry.document.blocks);

      const { editor } = harness("");
      const attachments = new Attachments();
      setComposerBody(editor, attachments, { html: entry.html });
      expect(emitted(documentBlocks(editor), attachments)).toEqual(saved);
    });
  }
});
