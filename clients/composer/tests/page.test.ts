// The page the message is written on: shown as the reading view shows it, sent as it was written.

import { describe, expect, test } from "bun:test";

import { documentBlocks } from "../src/document";
import { installHistory } from "../src/history";
import { asWritten, type ColourPiece, installPage, MAX_BATCH_BYTES, type PageScheme } from "../src/page";
import { writtenHtml } from "../src/page_regions";
import { setComposerQuote } from "../src/quote";
import { installRevisionCounter } from "../src/revision";
import { setComposerSignature, signatureBody } from "../src/signature";
import type { Block, QuoteValue } from "../src/types";
import { harness } from "./support";

const settled = () => new Promise((resolve) => setTimeout(resolve, 0));

const DARK = {
  scheme: "dark",
  background: "#1e1e1e",
  text: "#e3e3e3",
  link: "#8ab4f8",
  quote_rule: "#5c5c5c",
  header_rule: "#4a5567",
  table_rule: "#3d4146",
};
const LIGHT = {
  scheme: "light",
  background: "#ffffff",
  text: "#1a1a1a",
  link: "#2864d6",
  quote_rule: "#cccccc",
  header_rule: "rgb(181, 196, 223)",
  table_rule: "#d8dde3",
};

/// What the core answers on a dark page, for the pieces these tests use; every other piece, and
/// every piece on a light page, is shown as written.
const MOVED: Record<string, string> = {
  '{"ink":"#c00000"}': "#ff8a80",
  '{"fill":"#ffff00"}': "#5c5c00",
  '{"attribute":{"element":"p","attribute":"style","value":"color:#000000 !important"}}': "color:#e3e3e3 !important",
  '{"attribute":{"element":"td","attribute":"bgcolor","value":"#ffffff"}}': "#1e1e1e",
  '{"attribute":{"element":"font","attribute":"color","value":"#000080"}}': "#9ab0ff",
  '{"sheet":".x{color:#000}"}': ".x{color:#e3e3e3}",
};

const QUOTED =
  '<style>.x{color:#000}</style><table><tbody><tr><td bgcolor="#ffffff">' +
  '<p style="color:#000000 !important">Still on?</p><font color="#000080">Alice</font>' +
  "</td></tr></tbody></table>";

interface Asked {
  scheme: PageScheme;
  pieces: ColourPiece[];
}

function setup(body = "", reply: "answer" | "never" = "answer") {
  const h = harness(body);
  const asked: Asked[] = [];
  const history = installHistory(h.editor);
  const revision = installRevisionCounter(h.editor);
  const page = installPage(
    h.editor,
    async (scheme, pieces) => {
      asked.push({ scheme, pieces });
      if (reply === "never") return null;
      return pieces.map((piece) => (scheme === "dark" ? (MOVED[JSON.stringify(piece)] ?? null) : null));
    },
    (change) => history.unseen(() => revision.quietly(change)),
  );
  history.around((change) => page.asWritten(change));
  return { ...h, asked, history, revision, page };
}

function quoteOf(editor: HTMLElement): QuoteValue {
  const blocks: Block[] = documentBlocks(editor);
  const quote = blocks.find((block): block is { Quote: QuoteValue } => "Quote" in block);
  if (!quote) throw new Error("no Quote block");
  return quote.Quote;
}

async function dark(s: ReturnType<typeof setup>) {
  s.page.set(DARK);
  await settled();
  await settled();
}

describe("setComposerPage", () => {
  test("names the page and draws it in the core's colours", () => {
    const { editor, page } = setup();
    page.set(JSON.stringify(DARK));
    const root = editor.ownerDocument.documentElement;
    expect(root.dataset.page).toBe("dark");
    expect(root.style.getPropertyValue("--page")).toBe("#1e1e1e");
    expect(root.style.getPropertyValue("--page-link")).toBe("#8ab4f8");
    expect(root.style.getPropertyValue("--header-rule")).toBe("#4a5567");
    expect(page.scheme()).toBe("dark");
  });

  test("keeps the page it has for anything that is not one", () => {
    const { editor, page } = setup();
    page.set(LIGHT);
    for (const value of [null, "{", { ...DARK, scheme: "dim" }, { ...DARK, text: "red;}" }, { ...DARK, link: 1 }]) {
      page.set(value);
      expect(page.scheme()).toBe("light");
    }
    expect(editor.ownerDocument.documentElement.style.getPropertyValue("--text")).toBe("#1a1a1a");
  });
});

describe("the writer's own runs", () => {
  test("are shown moved by a rule on their stamp, and their markup is left alone", async () => {
    const run = '<span data-color="#c00000" style="color: #c00000;">red</span>';
    const s = setup(`<div>${run} <span data-highlight="#ffff00" style="background-color: #ffff00;">lit</span></div>`);
    await dark(s);
    const sheet = s.editor.ownerDocument.getElementById("page-colours")!.textContent!;
    expect(sheet).toContain('.editor [data-color="#c00000"]{color:#ff8a80!important}');
    expect(sheet).toContain('.editor [data-highlight="#ffff00"]{background-color:#5c5c00!important}');
    expect(s.editor.innerHTML).toContain(run);
  });

  test("are asked about once, and a colour added later is asked about when it appears", async () => {
    const s = setup('<div><span data-color="#c00000">red</span></div>');
    await dark(s);
    const span = s.editor.ownerDocument.createElement("span");
    span.dataset.color = "#00b050";
    s.editor.firstElementChild!.appendChild(span);
    await settled();
    await settled();
    const pieces = s.asked.flatMap((ask) => ask.pieces.map((piece) => JSON.stringify(piece)));
    expect(pieces.filter((piece) => piece === '{"ink":"#c00000"}')).toHaveLength(1);
    expect(pieces).toContain('{"ink":"#00b050"}');
  });

  test("on a light page carry no rule", async () => {
    const s = setup('<div><span data-color="#c00000">red</span></div>');
    await dark(s);
    s.page.set(LIGHT);
    await settled();
    await settled();
    expect(s.editor.ownerDocument.getElementById("page-colours")!.textContent).toBe("");
  });
});

describe("a quoted original", () => {
  async function quoted() {
    const s = setup();
    setComposerQuote(s.editor, { style: "Indented", body_html: QUOTED, body_plain: "Still on?" });
    await dark(s);
    return s;
  }

  test("is shown with every colour moved in place", async () => {
    const { editor } = await quoted();
    expect(editor.querySelector("p")!.getAttribute("style")).toBe("color:#e3e3e3 !important");
    expect(editor.querySelector("td")!.getAttribute("bgcolor")).toBe("#1e1e1e");
    expect(editor.querySelector("font")!.getAttribute("color")).toBe("#9ab0ff");
    expect(editor.querySelector("style")!.textContent).toBe(".x{color:#e3e3e3}");
  });

  test("is sent exactly as it was written", async () => {
    const { editor } = await quoted();
    expect(quoteOf(editor).body_html).toBe(QUOTED);
  });

  test("goes back to what was written on a light page, with nothing of the page left on it", async () => {
    const s = await quoted();
    s.page.set(LIGHT);
    await settled();
    await settled();
    expect(s.editor.querySelector(".aq-body")!.innerHTML).toBe(QUOTED);
  });

  test("keeps a declaration added to a moved element, and puts its colour back", async () => {
    const { editor } = await quoted();
    const p = editor.querySelector("p")!;
    p.setAttribute("style", "color:#e3e3e3 !important; font-weight: bold;");
    const sent = quoteOf(editor).body_html;
    expect(sent).toContain("font-weight: bold");
    expect(sent).toMatch(/color: (#000000|rgb\(0, 0, 0\)) !important/);
    expect(sent).not.toContain("e3e3e3");
  });

  test("an element put back by an undo after the page changed is shown for the page it is on now", async () => {
    const s = await quoted();
    const p = s.editor.querySelector("p")!;
    const parent = p.parentElement!;
    const next = p.nextSibling;
    p.remove();
    s.page.set(LIGHT);
    await settled();
    await settled();
    parent.insertBefore(p, next);
    await settled();
    await settled();
    expect(p.getAttribute("style")).toBe("color:#000000 !important");
    expect(p.hasAttribute("data-written-style")).toBe(false);
  });

  test("is no edit: neither a step to undo nor a revision", async () => {
    const s = setup();
    setComposerQuote(s.editor, { style: "Indented", body_html: QUOTED, body_plain: "" });
    await settled();
    const before = s.revision();
    const steps = s.history.steps();
    await dark(s);
    expect(s.editor.querySelector("td")!.getAttribute("bgcolor")).toBe("#1e1e1e");
    expect(s.revision()).toBe(before);
    expect(s.history.steps()).toEqual(steps);
  });

  test("is shown as written while the host gives no answer, and asked about again on the next change", async () => {
    const s = setup("", "never");
    setComposerQuote(s.editor, { style: "Indented", body_html: QUOTED, body_plain: "" });
    await dark(s);
    expect(s.editor.querySelector(".aq-body")!.innerHTML).toBe(QUOTED);
    const asked = s.asked.length;
    s.editor.querySelector("td")!.setAttribute("bgcolor", "#fefefe");
    await settled();
    await settled();
    expect(s.asked.length).toBeGreaterThan(asked);
  });
});

describe("a signature", () => {
  test("is shown moved and stored as written", async () => {
    const s = setup();
    const html = '<p style="color:#000000 !important">Alice</p>';
    setComposerSignature(s.editor, { body_html: html, body_plain: "Alice" });
    await dark(s);
    expect(s.editor.querySelector("p")!.getAttribute("style")).toBe("color:#e3e3e3 !important");
    const blocks: Block[] = documentBlocks(s.editor);
    const signature = blocks.find((block): block is { Signature: { body_html: string; body_plain: string } } => "Signature" in block);
    expect(signature?.Signature.body_html).toBe(html);
  });

  test("in the signature editor, the whole editor is kept as written", async () => {
    const html = '<p style="color:#000000 !important">Alice</p>';
    const s = setup(html);
    s.page.setWhole(true);
    await dark(s);
    expect(s.editor.querySelector("p")!.getAttribute("style")).toBe("color:#e3e3e3 !important");
    expect(signatureBody(s.editor).body_html).toBe(html);
  });
});

describe("asking the core", () => {
  test("goes in batches a request can carry", async () => {
    const big = (n: number) => `.n${n}{color:#000}${" ".repeat(MAX_BATCH_BYTES / 2)}`;
    const s = setup();
    setComposerQuote(s.editor, {
      style: "Indented",
      body_html: `<style>${big(1)}</style><style>${big(2)}</style><style>${big(3)}</style>`,
      body_plain: "",
    });
    await dark(s);
    expect(s.asked.length).toBeGreaterThanOrEqual(2);
    for (const ask of s.asked) {
      expect(JSON.stringify(ask.pieces).length).toBeLessThan(MAX_BATCH_BYTES + 1024);
    }
  });

  test("never, while no host has named a page", async () => {
    const s = setup('<div><span data-color="#c00000">red</span></div>');
    setComposerQuote(s.editor, { style: "Indented", body_html: QUOTED, body_plain: "" });
    await settled();
    expect(s.asked).toHaveLength(0);
  });
});

test("writtenHtml hands back markup the page never moved untouched", () => {
  const { editor } = harness('<p style="color:red">x</p>');
  expect(writtenHtml(editor)).toBe('<p style="color:red">x</p>');
});

describe("an edit on a dark page", () => {
  async function quoted() {
    const s = setup();
    setComposerQuote(s.editor, { style: "Indented", body_html: QUOTED, body_plain: "" });
    s.editor.firstElementChild!.innerHTML = '<span data-color="#c00000" style="color: #c00000;">red</span>';
    await dark(s);
    return s;
  }

  const input = (s: ReturnType<typeof setup>, type: string, inputType: string) => {
    const win = s.editor.ownerDocument.defaultView!;
    s.editor.dispatchEvent(new win.InputEvent(type, { inputType, cancelable: true }) as unknown as Event);
  };

  // An engine that moves content writes the computed style of what it moved into new markup; on a
  // dark page that would be the colour shown, and it would be sent.
  test("runs on the colours as written, and the page is dark again after it", async () => {
    const s = await quoted();
    const root = s.editor.ownerDocument.documentElement;
    const sheet = s.editor.ownerDocument.getElementById("page-colours")!;
    let during: Record<string, unknown> = {};
    asWritten(s.editor.ownerDocument, () => {
      during = {
        style: s.editor.querySelector("p")!.getAttribute("style"),
        sheet: s.editor.querySelector("style")!.textContent,
        runs: sheet.textContent,
        page: root.dataset.page,
        canvas: root.style.getPropertyValue("--page"),
      };
    });
    expect(during).toEqual({
      style: "color:#000000 !important",
      sheet: ".x{color:#000}",
      runs: "",
      page: undefined,
      canvas: "",
    });
    expect(s.editor.querySelector("p")!.getAttribute("style")).toBe("color:#e3e3e3 !important");
    expect(sheet.textContent).toContain("#ff8a80");
    expect(root.dataset.page).toBe("dark");
    expect(root.style.getPropertyValue("--page")).toBe("#1e1e1e");
  });

  test("from the engine runs on the colours as written until its input", async () => {
    const s = await quoted();
    const style = () => s.editor.querySelector("p")!.getAttribute("style");
    input(s, "beforeinput", "deleteContentBackward");
    expect(style()).toBe("color:#000000 !important");
    input(s, "input", "deleteContentBackward");
    expect(style()).toBe("color:#e3e3e3 !important");
  });

  test("typed at the caret moves nothing, so the page stays as it is", async () => {
    const s = await quoted();
    s.caretEnd("span");
    input(s, "beforeinput", "insertText");
    expect(s.editor.querySelector("p")!.getAttribute("style")).toBe("color:#e3e3e3 !important");
  });

  test("leaves the step being typed open", async () => {
    const s = await quoted();
    const span = s.editor.querySelector("span")!;
    span.firstChild!.textContent = "red!";
    await settled();
    const steps = s.history.steps();
    asWritten(s.editor.ownerDocument, () => {});
    await settled();
    expect(s.history.steps()).toEqual(steps);
    expect(s.history.undo()).toBe(true);
    expect(span.textContent).toBe("red");
    expect(s.editor.querySelector("p")!.getAttribute("style")).toBe("color:#e3e3e3 !important");
  });

  // Enter at a caret carries the caret's computed style to the new line as a typing style.
  test("Enter at a caret runs on the colours as written", async () => {
    const s = await quoted();
    s.caretEnd("span");
    input(s, "beforeinput", "insertParagraph");
    expect(s.editor.ownerDocument.getElementById("page-colours")!.textContent).toBe("");
    expect(s.editor.querySelector("p")!.getAttribute("style")).toBe("color:#000000 !important");
    input(s, "input", "insertParagraph");
    expect(s.editor.querySelector("p")!.getAttribute("style")).toBe("color:#e3e3e3 !important");
  });

  test("a character deleted inside one text node carries nothing, so the page stays", async () => {
    const s = await quoted();
    const text = s.editor.querySelector("span")!.firstChild!;
    s.editor.ownerDocument.defaultView!.getSelection()!.collapse(text, 2);
    input(s, "beforeinput", "deleteContentBackward");
    expect(s.editor.querySelector("p")!.getAttribute("style")).toBe("color:#e3e3e3 !important");
  });

  test("a copy serialises the colours as written, and the page returns after it", async () => {
    const s = await quoted();
    const win = s.editor.ownerDocument.defaultView!;
    s.editor.dispatchEvent(new win.Event("copy", { bubbles: true }) as unknown as Event);
    expect(s.editor.querySelector("p")!.getAttribute("style")).toBe("color:#000000 !important");
    await settled();
    expect(s.editor.querySelector("p")!.getAttribute("style")).toBe("color:#e3e3e3 !important");
  });

  test("an undo and a redo put back the colours as written", async () => {
    const s = await quoted();
    const td = () => s.editor.querySelector("td")!;
    asWritten(s.editor.ownerDocument, () => td().setAttribute("bgcolor", "#fefefe"));
    await settled();
    expect(s.history.undo()).toBe(true);
    await settled();
    await settled();
    expect(td().getAttribute("bgcolor")).toBe("#1e1e1e");
    expect(quoteOf(s.editor).body_html).toBe(QUOTED);
    expect(s.history.redo()).toBe(true);
    await settled();
    expect(quoteOf(s.editor).body_html).toBe(QUOTED.replace('bgcolor="#ffffff"', 'bgcolor="#fefefe"'));
  });
});

test("a quote's plain text leaves out its style sheets, so the page cannot reach it", async () => {
  const s = setup();
  setComposerQuote(s.editor, { style: "Indented", body_html: QUOTED, body_plain: "" });
  await dark(s);
  expect(quoteOf(s.editor).body_plain).toBe("Still on?\nAlice");
});
