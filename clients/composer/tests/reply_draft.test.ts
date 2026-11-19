// A drafted reply goes in above the signature and the quote as one step of the editor's history,
// and the editor tells its host after every change what the reply holds (`docs/ai.md` rules 21 to
// 23).

import { describe, expect, test } from "bun:test";

import { documentBlocks } from "../src/document";
import { installHistory } from "../src/history";
import { installReplyDraft, type ReplyDraftReport } from "../src/reply_draft";
import { harness } from "./support";

/// A MutationObserver delivers in a microtask.
const settled = () => new Promise((resolve) => setTimeout(resolve, 0));

const SIGNATURE = `<div class="allodia-signature"><p>Sam</p></div>`;
const QUOTE =
  `<div class="allodia-quote"><div class="aq-attr">On 1 July, Anna wrote:</div>` +
  `<div class="aq-body"><p>Are we on [date]?</p>` +
  `<div class="allodia-signature"><p>Anna</p></div></div></div>`;

function drafting(body: string) {
  const h = harness(body);
  const history = installHistory(h.editor);
  const told: ReplyDraftReport[] = [];
  const draft = installReplyDraft(h.editor, history, (report) => told.push(report));
  const last = () => told.at(-1);
  return { ...h, history, draft, told, last };
}

const seed = (serial: number, text: string, gaps: string[] = [], insert = true) => ({
  serial,
  text,
  gaps,
  insert,
});

describe("a drafted reply", () => {
  test("replaces the lead and leaves the signature and the quote where they are", () => {
    const d = drafting(`<p>typed by hand</p>${SIGNATURE}${QUOTE}`);
    expect(d.draft.setDraft(seed(1, "Hi Anna,\n\nYes, Friday works."))).toBe(true);
    expect(d.html()).toBe(`<div>Hi Anna,</div><div><br></div><div>Yes, Friday works.</div>${SIGNATURE}${QUOTE}`);
  });

  test("stops at the composer's own signature, not the one quoted inside the original", () => {
    const d = drafting(`<p>old</p>${QUOTE}`);
    d.draft.setDraft(seed(1, "Fine by me."));
    expect(d.editor.firstElementChild?.textContent).toBe("Fine by me.");
    expect(d.editor.querySelector(".allodia-quote .allodia-signature")?.textContent).toBe("Anna");
    expect(d.editor.children.length).toBe(2);
  });

  test("leaves the caret at its end, in the last list item when it ends with a list", () => {
    const d = drafting(`${SIGNATURE}${QUOTE}`);
    d.draft.setDraft(seed(1, "Points:\n- first\n- last"));
    expect(d.caretHost()?.textContent).toBe("last");
  });

  test("reads back out as paragraphs above the signature and the quote", () => {
    const d = drafting(`${SIGNATURE}${QUOTE}`);
    d.draft.setDraft(seed(1, "Hi Anna,\nYes."));
    expect(documentBlocks(d.editor).map((block) => Object.keys(block)[0])).toEqual([
      "Paragraph",
      "Paragraph",
      "Signature",
      "Quote",
    ]);
  });

  test("is one undo step: one undo takes it out whole, one redo brings it back", async () => {
    const d = drafting(`<div>my words</div>${SIGNATURE}`);
    d.draft.setDraft(seed(1, "Line one\nLine two\n- a point"));
    const drafted = d.html();
    expect(d.history.undo()).toBe(true);
    expect(d.html()).toBe(`<div>my words</div>${SIGNATURE}`);
    expect(d.history.undo()).toBe(false);
    expect(d.history.redo()).toBe(true);
    expect(d.html()).toBe(drafted);
    await settled();
  });

  test("a serial the editor already holds is not put in again", () => {
    const d = drafting(SIGNATURE);
    d.draft.setDraft(seed(2, "Second"));
    expect(d.draft.setDraft(seed(2, "Second"))).toBe(false);
    expect(d.draft.setDraft(seed(1, "First"))).toBe(false);
    expect(d.editor.firstElementChild?.textContent).toBe("Second");
    expect(d.draft.setDraft("not json")).toBe(false);
  });
});

describe("what the editor tells its host", () => {
  test("whether the lead is empty, the draft untouched, or written", async () => {
    const d = drafting(`<div><br></div>${SIGNATURE}${QUOTE}`);
    d.draft.resend();
    expect(d.last()?.lead).toBe("empty");
    d.draft.setDraft(seed(1, "Hi Anna,"));
    expect(d.last()).toMatchObject({ serial: 1, lead: "draft", inserted: true });
    d.editor.firstElementChild!.appendChild(d.editor.ownerDocument.createTextNode(" and more"));
    await settled();
    expect(d.last()?.lead).toBe("written");
  });

  test("the draft's gaps left, by their place, and an undo brings one back", async () => {
    const d = drafting(`${SIGNATURE}${QUOTE}`);
    d.draft.setDraft(seed(1, "On [date] at [time].", ["[date]", "[time]"]));
    expect(d.last()?.left).toEqual([0, 1]);
    const line = d.editor.firstElementChild!.firstChild as Text;
    line.data = "On Friday at [time].";
    await settled();
    expect(d.last()?.left).toEqual([1]);
    expect(d.last()?.lead).toBe("written");
    d.history.undo();
    await settled();
    expect(d.last()?.left).toEqual([0, 1]);
  });

  test("a gap in the quoted original does not count", () => {
    const d = drafting(`${SIGNATURE}${QUOTE}`);
    d.draft.setDraft(seed(1, "Fine by me.", ["[date]"]));
    expect(d.last()?.left).toEqual([]);
  });

  test("the insertion undone and redone, so the card can follow", async () => {
    const d = drafting(`<div>mine</div>${SIGNATURE}`);
    d.draft.setDraft(seed(1, "Drafted"));
    expect(d.last()?.inserted).toBe(true);
    d.history.undo();
    await settled();
    expect(d.last()).toMatchObject({ inserted: false, lead: "written" });
    d.history.redo();
    await settled();
    expect(d.last()).toMatchObject({ inserted: true, lead: "draft" });
  });

  test("a count of edits to the lead that grows with each, and nothing for a change below it", async () => {
    const d = drafting(`<div>a</div>${SIGNATURE}`);
    d.draft.resend();
    const before = d.last()!.edits;
    (d.editor.firstElementChild!.firstChild as Text).data = "ab";
    await settled();
    expect(d.last()!.edits).toBe(before + 1);
    const count = d.told.length;
    d.editor.querySelector(".allodia-signature p")!.textContent = "Samuel";
    await settled();
    expect(d.told.length).toBe(count);
  });

  test("a picture the person put above the signature is written text", () => {
    const d = drafting(`<div><img src="cid:a"></div>${SIGNATURE}`);
    d.draft.resend();
    expect(d.last()?.lead).toBe("written");
  });

  test("an editor opened later follows a draft already in its body without putting it in again", () => {
    const d = drafting(`<div>Hi [name],</div>${SIGNATURE}`);
    expect(d.draft.setDraft(seed(4, "Hi [name],", ["[name]"], false))).toBe(true);
    expect(d.html()).toBe(`<div>Hi [name],</div>${SIGNATURE}`);
    expect(d.last()).toMatchObject({ serial: 4, lead: "draft", inserted: true, left: [0] });
    expect(d.history.undo()).toBe(false);
  });

  test("carries no text of the reply", () => {
    const d = drafting(SIGNATURE);
    d.draft.setDraft(seed(1, "Secret words [gap]", ["[gap]"]));
    expect(JSON.stringify(d.told)).not.toContain("Secret");
    expect(JSON.stringify(d.told)).not.toContain("[gap]");
  });
});
