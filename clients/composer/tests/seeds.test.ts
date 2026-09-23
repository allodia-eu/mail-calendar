// A drafted reply goes into the open composer above the signature and the quote, and takes nothing
// else with it (`docs/ai.md`).

import { describe, expect, test } from "bun:test";

import { documentBlocks } from "../src/document";
import { composerLeadHasText, setComposerDraftText } from "../src/seeds";
import { harness } from "./support";

const SIGNATURE = `<div class="allodia-signature" data-signature-plain="Sam"><p>Sam</p></div>`;
const QUOTE =
  `<div class="allodia-quote"><div class="aq-attr">On 1 July, Anna wrote:</div>` +
  `<div class="aq-body"><p>Are we on?</p>` +
  `<div class="allodia-signature"><p>Anna</p></div></div></div>`;

describe("a drafted reply", () => {
  test("replaces the lead and leaves the signature and the quote where they are", () => {
    const { editor, html } = harness(`<p>typed by hand</p>${SIGNATURE}${QUOTE}`);

    setComposerDraftText(editor, "Hi Anna,\n\nYes, Friday works.");

    expect(html()).toBe(
      `<div>Hi Anna,</div><div><br></div><div>Yes, Friday works.</div>${SIGNATURE}${QUOTE}`,
    );
  });

  test("stops at the composer's own signature, not the one quoted inside the original", () => {
    const { editor } = harness(`<p>old</p>${QUOTE}`);

    setComposerDraftText(editor, "Fine by me.");

    // No signature of our own: the boundary is the quote, and the quoted signature is untouched.
    expect(editor.firstElementChild?.textContent).toBe("Fine by me.");
    expect(editor.querySelector(".allodia-quote .allodia-signature")?.textContent).toBe("Anna");
    expect(editor.children.length).toBe(2);
  });

  test("fills a body that has neither", () => {
    const { editor, html } = harness("<p>old</p>");
    setComposerDraftText(editor, "One\nTwo");
    expect(html()).toBe("<div>One</div><div>Two</div>");
  });

  test("is text, so markup in a draft cannot become an element", () => {
    const { editor } = harness(SIGNATURE);
    setComposerDraftText(editor, "<img src=x onerror=alert(1)>");
    expect(editor.querySelector("img")).toBeNull();
    expect(editor.firstElementChild?.textContent).toBe("<img src=x onerror=alert(1)>");
  });

  test("leaves the caret at the end of the draft", () => {
    const { editor, caretHost } = harness(`${SIGNATURE}${QUOTE}`);
    setComposerDraftText(editor, "First\nLast line");
    expect(caretHost()?.textContent).toBe("Last line");
  });

  test("reads back out as paragraphs above the signature and the quote", () => {
    const { editor } = harness(`${SIGNATURE}${QUOTE}`);
    setComposerDraftText(editor, "Hi Anna,\nYes.");
    const kinds = documentBlocks(editor).map((block) => Object.keys(block)[0]);
    expect(kinds).toEqual(["Paragraph", "Paragraph", "Signature", "Quote"]);
  });
});

describe("the draft's id", () => {
  test("is kept by the editor, and a later draft replaces it", () => {
    const { editor } = harness(SIGNATURE);
    setComposerDraftText(editor, "First", "draft-1");
    expect(editor.dataset.aiDraft).toBe("draft-1");
    setComposerDraftText(editor, "Second", "draft-2");
    expect(editor.dataset.aiDraft).toBe("draft-2");
  });

  test("is not set by a draft that came without one", () => {
    const { editor } = harness(SIGNATURE);
    setComposerDraftText(editor, "No id");
    expect(editor.dataset.aiDraft).toBeUndefined();
  });
});

describe("whether the person has written a reply", () => {
  test("is no above an empty line, the signature and the quote", () => {
    const { editor } = harness(`<div><br></div>${SIGNATURE}${QUOTE}`);
    expect(composerLeadHasText(editor)).toBe(false);
  });

  test("is yes once there is text above them", () => {
    const { editor } = harness(`<div>Thanks,</div>${SIGNATURE}${QUOTE}`);
    expect(composerLeadHasText(editor)).toBe(true);
  });

  test("does not count the text of the signature or the quote", () => {
    const { editor } = harness(`${SIGNATURE}${QUOTE}`);
    expect(composerLeadHasText(editor)).toBe(false);
  });

  test("counts a picture the person put there", () => {
    const { editor } = harness(`<div><img src="cid:a"></div>${SIGNATURE}`);
    expect(composerLeadHasText(editor)).toBe(true);
  });
});
