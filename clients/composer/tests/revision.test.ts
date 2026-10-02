import { describe, expect, test } from "bun:test";

import { installRevisionCounter } from "../src/revision";
import { harness } from "./support";

/// A MutationObserver delivers in a microtask, so an assertion has to let the queue drain first.
const settled = () => new Promise((resolve) => setTimeout(resolve, 0));

describe("the revision counter", () => {
  test("starts at zero and does not move on its own", async () => {
    const { editor } = harness("<p>Half a sentence</p>");
    const revision = installRevisionCounter(editor);
    expect(revision()).toBe(0);
    await settled();
    expect(revision()).toBe(0);
  });

  test("counts typing into the message", async () => {
    const { editor } = harness("<p>Half a sentence</p>");
    const revision = installRevisionCounter(editor);
    const paragraph = editor.querySelector("p")!;
    paragraph.firstChild!.textContent = "Half a sentence, then the rest";
    await settled();
    expect(revision()).toBeGreaterThan(0);
  });

  test("counts a change made to the document rather than typed", async () => {
    // What an `input` listener would miss: the toolbar, a pasted picture and a dropped file all
    // write to the DOM, and a host that stopped seeing those would save the draft without them.
    const { editor } = harness("<p>Half a sentence</p>");
    const revision = installRevisionCounter(editor);
    const image = editor.ownerDocument.createElement("img");
    image.src = "data:image/png;base64,iVBORw0KGgo=";
    editor.appendChild(image);
    await settled();
    expect(revision()).toBeGreaterThan(0);
  });

  test("keeps counting, so a host can tell one change from the next", async () => {
    const { editor } = harness("<p>Half a sentence</p>");
    const revision = installRevisionCounter(editor);
    const paragraph = editor.querySelector("p")!;
    paragraph.firstChild!.textContent = "one";
    await settled();
    const after = revision();
    paragraph.firstChild!.textContent = "two";
    await settled();
    expect(revision()).toBeGreaterThan(after);
  });
});
