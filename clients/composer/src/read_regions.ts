// Finding the quoted original and the signature in a stored body, whichever client wrote it.
//
// Each becomes the region the editor holds it in (`.allodia-quote`, `.allodia-signature`), so a
// reopened draft's quote can switch style and its signature can follow a change of From, as one
// written here does. Each client marks the two in markup of its own:
//
// - this app: an indented `<blockquote>` with its own left border, or a header block under a
//   top-bordered divider; the signature in `.allodia-signature`;
// - Thunderbird: `.moz-cite-prefix` over `<blockquote type="cite">`, a forward in
//   `.moz-forward-container` with its headers in `.moz-email-headers-table`, the signature in
//   `.moz-signature`;
// - Gmail: `.gmail_attr` over `<blockquote class="gmail_quote">`;
// - Outlook: a top-bordered block of bold `Label:` lines (`#divRplyFwdMsg` on the web), the original
//   following it; the signature in `#Signature`, or in Word's paragraphs bookmarked
//   `_MailAutoSig`.
//
// A `<blockquote>` none of them marks is an indent, the way Thunderbird's editor writes one, and
// `lines.ts` reads it as one.

import { quoteContainer } from "./quote";
import { signatureContainer } from "./signature";
import type { QuoteHeader, QuoteStyle } from "./types";

export const REGIONS = ".allodia-quote, .allodia-signature";

/// The indented quote's left border, which only this app's renderer writes (`INDENTED_QUOTE_STYLE`
/// in `crates/mailcal-composer`): the mark that the paragraph before it is that quote's attribution
/// rather than the user's last line.
const OWN_INDENTED_BORDER = "border-left:2px solid #cccccc";

/// The colour of the divider over a line-and-header quote's header block, which this app's renderer
/// writes (`HEADER_DIVIDER_OPEN`).
const OWN_HEADER_DIVIDER = "rgb(181, 196, 223)";

function styleOf(element: Element): string {
  return (element.getAttribute("style") ?? "").replace(/\s+/g, " ").replace(/:\s/g, ":");
}

/// Turns each quoted original into a quote region, outermost first and in one pass, so a message
/// quoted inside it, which carries its own quote and perhaps its own signature, stays part of the
/// quoted markup rather than being read as a second region of this message.
export function readQuotes(body: HTMLElement): void {
  for (const element of Array.from(body.querySelectorAll("blockquote, div"))) {
    if (!element.isConnected || element.closest(REGIONS)) continue;
    if (element.tagName === "BLOCKQUOTE") readBlockquote(element);
    else readDiv(element);
  }
}

function readBlockquote(element: Element): void {
  if (styleOf(element).includes(OWN_INDENTED_BORDER)) {
    const attribution = element.previousElementSibling;
    let line = "";
    // A `<div>`, or a `<p>` in a draft saved before lines and paragraphs were told apart.
    if (attribution?.tagName === "DIV" || attribution?.tagName === "P") {
      line = attribution.textContent ?? "";
      attribution.remove();
    }
    replaceWithQuote(element, "Indented", line, [], childrenOf(element));
    return;
  }
  if (element.parentElement?.closest("li, td, th")) return;
  const cited =
    element.getAttribute("type")?.toLowerCase() === "cite" || element.classList.contains("gmail_quote");
  if (!cited) return;
  const prefix = element.previousElementSibling;
  let line = "";
  if (prefix?.matches(".moz-cite-prefix, .gmail_attr")) {
    line = flatText(prefix);
    prefix.remove();
  }
  replaceWithQuote(element, "Indented", line, [], childrenOf(element));
}

function readDiv(element: Element): void {
  if (styleOf(element).includes(OWN_HEADER_DIVIDER)) {
    const quoted = element.nextElementSibling;
    if (quoted?.tagName !== "DIV") return;
    const headers = headersOf(element, true);
    element.remove();
    replaceWithQuote(quoted, "LineAndHeader", "", headers, childrenOf(quoted));
    return;
  }
  if (element.classList.contains("moz-forward-container")) {
    readThunderbirdForward(element);
    return;
  }
  if (isOutlookHeader(element)) readOutlookHeader(element);
}

/// Thunderbird's forward: a separator line, the original's headers as a table, then its body, all
/// inside one container. The separator is Thunderbird's own words for "forwarded message", which
/// the header block replaces.
function readThunderbirdForward(container: Element): void {
  const table = container.querySelector(":scope > table.moz-email-headers-table");
  if (!table) return;
  const headers: QuoteHeader[] = [];
  for (const row of Array.from(table.querySelectorAll("tr"))) {
    const label = row.querySelector("th");
    const value = row.querySelector("td");
    if (!label || !value) continue;
    headers.push({ label: flatText(label).replace(/:$/, "").trim(), value: flatText(value) });
  }
  while (container.firstChild && container.firstChild !== table) container.firstChild.remove();
  table.remove();
  dropLeadingBreaks(container);
  replaceWithQuote(container, "LineAndHeader", "", headers, childrenOf(container));
}

/// Outlook's header block over a quoted original: a top border, then two or more bold `Label:`
/// lines (From, Sent, To, Subject in the sender's language).
function isOutlookHeader(element: Element): boolean {
  if (element.id === "divRplyFwdMsg") return true;
  if (!/border-top:[^;]*solid/i.test(styleOf(element))) return false;
  return headersOf(element, false).length >= 2;
}

/// The original follows Outlook's header block rather than sitting inside it, so the quote is the
/// block and everything after it. On the web a rule sits above the block, which the quote's own
/// divider replaces.
function readOutlookHeader(header: Element): void {
  const headers = headersOf(header, false);
  if (headers.length === 0) return;
  let anchor: Element = header;
  while (anchor.parentElement && anchor.parentElement.tagName !== "BODY" && onlyElement(anchor.parentElement, anchor)) {
    anchor = anchor.parentElement;
  }
  const rule = anchor.previousElementSibling;
  if (rule?.tagName === "HR") rule.remove();
  const quoted: Node[] = [];
  for (let node = anchor.nextSibling; node; node = node.nextSibling) quoted.push(node);
  replaceWithQuote(anchor, "LineAndHeader", "", headers, quoted);
}

function onlyElement(parent: Element, child: Element): boolean {
  return Array.from(parent.childNodes).every(
    (node) => node === child || (node.nodeType === 3 && !(node.nodeValue ?? "").trim()),
  );
}

function childrenOf(element: Element): Node[] {
  return Array.from(element.childNodes);
}

/// Replaces `element` with a quote region holding `content`.
function replaceWithQuote(
  element: Element,
  style: QuoteStyle,
  line: string,
  headers: QuoteHeader[],
  content: Node[],
): void {
  const region = quoteContainer(element.ownerDocument, { style, attribution: { line, headers } });
  const quoted = region.querySelector(".aq-body")!;
  quoted.textContent = "";
  for (const node of content) quoted.appendChild(node);
  element.replaceWith(region);
}

/// The `<strong>Label: </strong>value<br>` lines of a header block. This app's own (`own`) read
/// back exactly as they were written; another client's are any bold run ending in a colon, their
/// values with the whitespace their markup wrapped them in collapsed.
function headersOf(divider: Element, own: boolean): QuoteHeader[] {
  const headers: QuoteHeader[] = [];
  let current: QuoteHeader | null = null;
  const walk = (node: Node) => {
    for (const child of Array.from(node.childNodes)) {
      const tag = child.nodeType === 1 ? (child as Element).tagName : null;
      if ((tag === "STRONG" || tag === "B") && (own || /:\s*$/.test(child.textContent ?? ""))) {
        current = { label: (child.textContent ?? "").replace(/:\s*$/, "").trim(), value: "" };
        headers.push(current);
      } else if (tag === "BR" || tag === "P" || tag === "DIV") {
        current = null;
        if (tag !== "BR") walk(child);
        current = null;
      } else if (tag) {
        walk(child);
      } else if (current) {
        current.value += child.textContent ?? "";
      }
    }
  };
  walk(divider);
  if (!own) for (const header of headers) header.value = header.value.replace(/\s+/g, " ").trim();
  return headers;
}

function flatText(element: Element): string {
  return (element.textContent ?? "").replace(/\s+/g, " ").trim();
}

function dropLeadingBreaks(container: Element): void {
  for (let node = container.firstChild; node; node = container.firstChild) {
    const blank = node.nodeType === 3 && !(node.nodeValue ?? "").trim();
    const br = node.nodeType === 1 && (node as Element).tagName === "BR";
    if (!blank && !br) return;
    node.remove();
  }
}

/// The signatures other clients mark, outside any quote: Thunderbird's and Outlook on the web's
/// containers, and the run of paragraphs Word bookmarks as its own.
export function readSignatures(body: HTMLElement): void {
  const containers = body.querySelectorAll("div.moz-signature, div#Signature, div[id^='Signature_']");
  for (const element of Array.from(containers)) {
    if (!element.isConnected || element.closest(REGIONS)) continue;
    replaceWithSignature([element], element.innerHTML);
  }
  readWordSignature(body);
}

/// Word marks the signature it inserted with a bookmark, `_MailAutoSig`: an `<a name>` on its first
/// line and a `mso-bookmark` style on every line after it. The lines in that run become the region.
function readWordSignature(body: HTMLElement): void {
  const markers = Array.from(body.querySelectorAll("a[name='_MailAutoSig'], [style*='_MailAutoSig']")).filter(
    (marker) => !marker.closest(REGIONS),
  );
  const lines: Element[] = [];
  for (const marker of markers) {
    const line = marker.closest("p, div, h1, h2, h3, h4, h5, h6");
    if (line && !line.closest(REGIONS) && !lines.includes(line) && !line.querySelector("p, div")) lines.push(line);
    else if (!line && !(marker.textContent ?? "").trim()) marker.remove();
  }
  if (lines.length === 0) return;
  const parent = lines[0]!.parentElement;
  const run = lines.filter((line) => line.parentElement === parent);
  replaceWithSignature(run, run.map((line) => line.outerHTML).join(""));
}

function replaceWithSignature(elements: Element[], html: string): void {
  const first = elements[0]!;
  const region = signatureContainer(first.ownerDocument, { body_html: html, body_plain: "" });
  first.replaceWith(region);
  for (const element of elements.slice(1)) element.remove();
}
