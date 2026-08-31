// The quoted original on a reply or forward.
//
// The attribution is display-only; the body stays editable so the user can trim it. Both
// renderings ride on the container's dataset, so flipping the style is a re-render here rather
// than a round-trip to the core.

import { caretInto, documentOf, focusEditor } from "./dom";
import type { Block, QuoteAttribution, QuoteHeader, QuoteStyle, QuoteValue } from "./types";

/// The seed a host passes to `setComposerQuote`. `initial_text` pre-fills the lead paragraph and
/// is assigned as TEXT, never parsed as markup (`docs/composer-security.md`, Gate 11).
export interface QuoteSeed extends Partial<QuoteValue> {
  initial_text?: string;
}

function readStyle(value: string | undefined): QuoteStyle {
  return value === "LineAndHeader" ? "LineAndHeader" : "Indented";
}

function readHeaders(json: string | undefined): QuoteHeader[] {
  try {
    const parsed: unknown = JSON.parse(json || "[]");
    return Array.isArray(parsed) ? (parsed as QuoteHeader[]) : [];
  } catch {
    return [];
  }
}

/// The `Quote` block emitted to the shared composer: its style, the fixed attribution (the core
/// re-renders the visible line or header block from these on send), the editable body's current
/// HTML, and the original plain text for the `text/plain` part.
export function quoteBlock(container: HTMLElement): Block {
  const body = container.querySelector(".aq-body");
  return {
    Quote: {
      style: readStyle(container.dataset.quoteStyle),
      attribution: {
        line: container.dataset.quoteLine || "",
        headers: readHeaders(container.dataset.quoteHeaders),
      },
      body_html: body ? body.innerHTML : "",
      body_plain: container.dataset.quotePlain || "",
    },
  };
}

/// Builds the display-only attribution above the quoted body for the current style: the one-line
/// "On … wrote:", or the labelled From/Sent/To/Subject block. Text is set via `textContent` so an
/// address like `Alice <a@x>` can never inject markup.
export function renderQuoteAttribution(container: HTMLElement): void {
  const attr = container.querySelector(".aq-attr");
  if (!attr) return;
  const doc = documentOf(container);
  attr.textContent = "";
  if (container.dataset.quoteStyle === "LineAndHeader") {
    readHeaders(container.dataset.quoteHeaders).forEach((header, index) => {
      if (index > 0) attr.appendChild(doc.createElement("br"));
      const label = doc.createElement("strong");
      label.textContent = `${header.label}: `;
      attr.appendChild(label);
      attr.appendChild(doc.createTextNode(header.value || ""));
    });
  } else {
    attr.textContent = container.dataset.quoteLine || "";
  }
}

/// Seeds the editor for a reply or forward: a paragraph for the user's message above the quoted
/// original.
export function setComposerQuote(editor: HTMLElement, seed: QuoteSeed): void {
  const doc = documentOf(editor);
  editor.innerHTML = "";

  const initialText = typeof seed.initial_text === "string" ? seed.initial_text : "";
  const lead = doc.createElement("p");
  if (initialText) lead.textContent = initialText;
  else lead.appendChild(doc.createElement("br"));
  editor.appendChild(lead);

  const attribution: QuoteAttribution = seed.attribution ?? { line: "", headers: [] };
  const container = doc.createElement("div");
  container.className = "allodia-quote";
  container.dataset.quoteStyle = readStyle(seed.style);
  container.dataset.quoteLine = attribution.line || "";
  container.dataset.quoteHeaders = JSON.stringify(attribution.headers || []);
  container.dataset.quotePlain = seed.body_plain || "";

  const attr = doc.createElement("div");
  attr.className = "aq-attr";
  attr.setAttribute("contenteditable", "false");
  container.appendChild(attr);

  const body = doc.createElement("div");
  body.className = "aq-body";
  if (seed.body_html) {
    body.innerHTML = seed.body_html;
  } else {
    // A plain-text original. Falling through to an empty `innerHTML` here is how the quoted message
    // used to VANISH: a message with no `text/html` part arrives with `body_html` empty, and the
    // reply then showed "On … wrote:" followed by nothing. The original was still on the wire (it
    // rides in `body_plain` into the `text/plain` part), which is exactly why it went unnoticed,
    // invisible to a MIME check, obvious to a person, who sees their reply lose the message they
    // answered.
    //
    // Rendered as TEXT, never markup: this is the one place a quoted body is not already sanitized
    // HTML from the core, and `textContent` cannot introduce an element whatever it contained.
    body.textContent = seed.body_plain || "";
    body.classList.add("aq-plain");
  }
  container.appendChild(body);

  renderQuoteAttribution(container);
  editor.appendChild(container);

  // The caret goes in the lead paragraph, above the quote; at its end when it was pre-filled, so
  // the user carries on typing after the seeded text.
  caretInto(lead, !initialText);
  focusEditor(editor);
}

/// Flips the quoted original between the two styles in place, preserving the user's message and
/// any trimming of the body; only the attribution and indent change.
export function setComposerQuoteStyle(editor: HTMLElement, style: string): void {
  const container = editor.querySelector<HTMLElement>(".allodia-quote");
  if (!container) return;
  container.dataset.quoteStyle = readStyle(style);
  renderQuoteAttribution(container);
}
