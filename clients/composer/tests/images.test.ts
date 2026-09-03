import { describe, expect, test } from "bun:test";

import { Attachments } from "../src/attachments";
import { documentBlocks, referencedAttachmentIds } from "../src/document";
import { imageFilesFrom, insertCapturedImage } from "../src/images";
import type { DraftAttachment } from "../src/types";
import { harness } from "./support";

/// A 1×1 transparent PNG, the shape a pasted screenshot arrives in.
const PNG =
  "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg==";

function inlineOf(attachment: DraftAttachment): { cid: string } | null {
  return typeof attachment.disposition === "string" ? null : attachment.disposition.Inline;
}

describe("a captured picture", () => {
  test("carries its own bytes and a content id, with no host blob handle", () => {
    const { editor, caret } = harness("<p>Look: </p>");
    caret("p");
    const attachments = new Attachments();

    expect(insertCapturedImage(editor, attachments, { data_url: PNG })).toBe(true);

    const [attachment, ...rest] = attachments.list();
    expect(rest).toHaveLength(0);
    // Exactly one source of bytes: Rust rejects a document that names both or neither.
    expect(attachment!.data_url).toBe(PNG);
    expect(attachment!.blob).toBeUndefined();
    expect(attachment!.media_type).toBe("image/png");
    expect(inlineOf(attachment!)?.cid).toMatch(/^img0\.\d+@/);
  });

  test("is emitted as an inline image the manifest backs", () => {
    const { editor, caret } = harness("<p>Look: </p>");
    caret("p");
    const attachments = new Attachments();
    insertCapturedImage(editor, attachments, { data_url: PNG, alt_text: "screenshot" });

    const blocks = documentBlocks(editor);
    const referenced = referencedAttachmentIds(blocks);
    expect([...referenced]).toEqual([attachments.list()[0]!.id]);
    expect(attachments.list(referenced)).toHaveLength(1);
  });

  test("with no caret lands above the signature and the quoted original", () => {
    // A drop arrives while the user was dragging rather than typing, so the editor may hold no
    // caret at all. Appending to the end of the document would put the picture below a reply's
    // signature and below the message it is replying to.
    const { editor } = harness(
      '<p>Reply</p><div class="allodia-signature">Alice</div>' +
        '<div class="allodia-quote"><div class="aq-body"><p>Original</p></div></div>',
    );
    const attachments = new Attachments();

    expect(insertCapturedImage(editor, attachments, { data_url: PNG })).toBe(true);

    const children = Array.from(editor.children).map((child) => child.className || child.tagName);
    expect(children).toEqual(["P", "allodia-signature", "allodia-quote"]);
    expect(editor.querySelector("p > img")).not.toBeNull();
  });

  test("is refused when the URI is not a picture", () => {
    // The same check the core repeats on submit: a `data:text/html` behind an `<img>` would be an
    // executable document attached as an image part.
    const { editor } = harness("<p>x</p>");
    const attachments = new Attachments();

    expect(
      insertCapturedImage(editor, attachments, { data_url: "data:text/html;base64,PHNjcmlwdD4=" }),
    ).toBe(false);
    expect(insertCapturedImage(editor, attachments, { data_url: "https://x.test/a.png" })).toBe(
      false,
    );
    // An SVG is a picture to the platform and script-capable to a reader, so it is not one of the
    // formats a body may carry. Nothing sniffs bytes on the clipboard, so the check is the type.
    expect(
      insertCapturedImage(editor, attachments, {
        data_url: "data:image/svg+xml;base64,PHN2Zz48L3N2Zz4=",
      }),
    ).toBe(false);
    expect(attachments.list()).toHaveLength(0);
  });

  test("is not read off the clipboard when its format is not one a body may carry", () => {
    // `imageFilesFrom` is the paste path's filter: an SVG on the clipboard is left to the plain
    // text branch rather than becoming an inline part.
    const svg = new File(["<svg/>"], "logo.svg", { type: "image/svg+xml" });
    const png = new File(["x"], "shot.png", { type: "image/png" });
    const transfer = {
      items: [],
      files: [svg, png],
    } as unknown as DataTransfer;

    expect(imageFilesFrom(transfer)).toEqual([png]);
  });

  test("gets a fresh id per picture, so two pastes are two parts", () => {
    const { editor, caret } = harness("<p>x</p>");
    caret("p");
    const attachments = new Attachments();
    insertCapturedImage(editor, attachments, { data_url: PNG });
    insertCapturedImage(editor, attachments, { data_url: PNG });

    const ids = attachments.list().map((item) => item.id);
    expect(new Set(ids).size).toBe(2);
    const cids = attachments.list().map((item) => inlineOf(item)?.cid);
    expect(new Set(cids).size).toBe(2);
  });
});

describe("the manifest", () => {
  test("drops an inline picture the body no longer references", () => {
    // Deleting a pasted picture must not make the message unsendable: Rust rejects an inline
    // attachment nothing points at, so the manifest is pruned to what the blocks emitted.
    const { editor, caret } = harness("<p>Look: </p>");
    caret("p");
    const attachments = new Attachments();
    insertCapturedImage(editor, attachments, { data_url: PNG });
    editor.querySelector("img")!.remove();

    expect(attachments.list(referencedAttachmentIds(documentBlocks(editor)))).toHaveLength(0);
  });

  test("drops a picture that ended up inside a quoted original", () => {
    // A quote travels as raw HTML, so an `<img>` in it is never emitted as a document node and
    // its attachment would dangle.
    const { editor, caret } = harness(
      '<p>Reply</p><div class="allodia-quote"><div class="aq-body"><p>Original</p></div></div>',
    );
    caret(".aq-body p");
    const attachments = new Attachments();
    insertCapturedImage(editor, attachments, { data_url: PNG });

    expect(attachments.list()).toHaveLength(1);
    expect(attachments.list(referencedAttachmentIds(documentBlocks(editor)))).toHaveLength(0);
  });

  test("keeps a regular attachment nothing in the body references", () => {
    // A file attachment stands on its own; only inline pictures are tied to an `<img>`.
    const attachments = new Attachments();
    attachments.add({
      id: "file-1",
      blob: "blob://file",
      file_name: "report.pdf",
      media_type: "application/pdf",
    });

    expect(attachments.list(new Set())).toHaveLength(1);
  });
});
