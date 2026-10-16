import { describe, expect, test } from "bun:test";

import { Attachments } from "../src/attachments";
import { pastedHtml, pasteContent } from "../src/paste";
import { harness } from "./support";

// A one-pixel PNG, the shape a picture copied from inside a draft has on the clipboard: the editor
// carries its pictures inline as `data:` URIs (images.ts).
const PNG =
  "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==";

function transfer(html: string): DataTransfer {
  return {
    getData: (type: string) => (type === "text/html" ? html : ""),
    items: [],
    files: [],
  } as unknown as DataTransfer;
}

describe("a picture inside pasted markup", () => {
  test("one the markup carries inline arrives with its text, backed by an inline attachment", () => {
    const { editor } = harness("");
    const attachments = new Attachments();
    const markup = pastedHtml(
      editor.ownerDocument,
      `<p>Before <img data-attachment-id="captured-7" src="${PNG}" alt="logo"> after</p>`,
      false,
      attachments,
    )!;

    editor.innerHTML = markup;
    const picture = editor.querySelector("img")!;
    expect(editor.textContent).toBe("Before  after");
    expect(picture.getAttribute("src")).toBe(PNG);
    expect(picture.getAttribute("alt")).toBe("logo");
    // Registered afresh rather than trusted: an id from the clipboard names an attachment of
    // another draft, or of none.
    const id = picture.dataset.attachmentId!;
    expect(id).not.toBe("captured-7");
    const [attachment] = attachments.list(new Set([id]));
    expect(attachment).toMatchObject({ id, media_type: "image/png", data_url: PNG });
    expect(attachment!.disposition).toHaveProperty("Inline");
  });

  test("a picture on its own is kept too, so it does not fall back to the plain text", () => {
    const { editor } = harness("");
    const content = pasteContent(editor.ownerDocument, transfer(`<img src="${PNG}">`), false, new Attachments());
    expect("html" in content && content.html).toContain("<img");
  });

  test("one named by URL, on the web or on disk, is dropped: nothing is fetched or read", () => {
    const { editor } = harness("");
    const attachments = new Attachments();
    const markup = pastedHtml(
      editor.ownerDocument,
      '<p>a <img src="https://example.com/a.png"> b <img src="file:///C:/Users/x/clip_image001.png"> c</p>',
      false,
      attachments,
    );
    expect(markup).toBe("a b c");
    expect(attachments.list()).toEqual([]);
  });

  test("a format a message may not carry, or a picture past the cap, is dropped", () => {
    const { editor } = harness("");
    const attachments = new Attachments();
    const svg = "data:image/svg+xml;base64,PHN2ZyB4bWxucz0naHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmcnLz4=";
    const huge = `data:image/png;base64,${"A".repeat(28 * 1024 * 1024)}`;
    const markup = pastedHtml(
      editor.ownerDocument,
      `<p>a <img src="${svg}"> b <img src="${huge}"> c</p>`,
      false,
      attachments,
    );
    expect(markup).toBe("a b c");
    expect(attachments.list()).toEqual([]);
  });

  test("a data URI the core would refuse on submit is dropped, so it cannot fail the send", () => {
    const { editor } = harness("");
    const attachments = new Attachments();
    const payload = PNG.slice(PNG.indexOf(",") + 1);
    const refused = [
      `data:image/png;name=logo.png;base64,${payload}`,
      `data:IMAGE/PNG;base64,${payload}`,
      "data:image/png,%89PNG",
    ];
    const markup = pastedHtml(
      editor.ownerDocument,
      `<p>a ${refused.map((url) => `<img src="${url}">`).join(" ")} b</p>`,
      false,
      attachments,
    );
    expect(markup).toBe("a b");
    expect(attachments.list()).toEqual([]);
  });

  test("without an attachment list to back it, a picture is dropped rather than left dangling", () => {
    const { editor } = harness("");
    expect(pastedHtml(editor.ownerDocument, `<p>a <img src="${PNG}"> b</p>`)).toBe("a b");
  });
});
