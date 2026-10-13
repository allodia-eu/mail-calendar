// The editor's entry point: resolves the singletons, wires the events, and installs the `window.*`
// seams the four native hosts drive the composer through.
//
// This is the only module that touches the `document`/`window` globals. Everything else takes the
// editor element and derives what it needs from it, which is what lets the commands be tested
// against an isolated DOM.

import { Attachments, type AttachmentMeta, type InlineImageMeta, insertAtCaret, safePreviewUrl } from "./attachments";
import { autoformatBulletList } from "./autoformat";
import { documentBlocks, referencedAttachmentIds } from "./document";
import { ancestorOf, focusEditor, rangeWithin, saveSelection } from "./dom";
import { applyMark } from "./format";
import { installNativeChrome } from "./host";
import { HostRequests } from "./host_requests";
import { type CapturedImage, insertCapturedImage, insertImageFiles } from "./images";
import { DEFAULT_LABELS, type Labels, mergeLabels } from "./labels";
import { editLinkThroughHost } from "./link_menu";
import { autolinkBeforeCaret } from "./links";
import { indentSelection } from "./lists";
import { pastedHtml, pasteContent } from "./paste";
import {
  completeComposerQuoteAttribution,
  setComposerQuote,
  setComposerQuoteStyle,
  type QuoteSeed,
} from "./quote";
import { installImageResize } from "./resize";
import { installRevisionCounter } from "./revision";
import { type BodySeed, focusComposerBody, setComposerBody, setPlainText } from "./seeds";
import {
  routeClickBelowSignature,
  setComposerSignature,
  setSignatureBody,
  signatureBody,
} from "./signature";
import { moveToAdjacentCell } from "./tables";
import { installToolbar } from "./toolbar";
import type { QuoteAttribution, SignatureValue } from "./types";

const doc = document;
const editor = doc.getElementById("editor") as HTMLElement;
const toolbarRoot = doc.querySelector(".toolbar") as HTMLElement;
const attachments = new Attachments();

const hostRequests = new HostRequests(window);

let labels: Labels = DEFAULT_LABELS;
const toolbar = installToolbar(editor, toolbarRoot, () => labels, () => {
  if (!hostRequests.answers("link")) return false;
  void editLinkThroughHost(editor, hostRequests);
  return true;
});
const chrome = installNativeChrome(editor, toolbarRoot);
installImageResize(editor);
const revision = installRevisionCounter(editor);

// Paste. Formatted text keeps what the document can hold of its formatting (`paste.ts`), and wins
// over a picture beside it, which is a rendering of that text (`pasteContent`). A picture on its
// own goes into the body where the caret is, as an inline image the core turns into a `cid:` part
// on send: what Outlook does with a pasted screenshot, and what the user means by pasting one into
// a message. Anything else arrives as plain text. A paste that asks for plain text (Paste and Match
// Style) carries no `text/html`, so it stays plain.
//
// The picture is read asynchronously, so the caret is captured first: without it a slow read would
// drop the image wherever the selection had moved to by the time the bytes arrived.
editor.addEventListener("paste", (event) => {
  event.preventDefault();
  const content = pasteContent(doc, event.clipboardData, caretHoldsInlineOnly());
  if ("images" in content) {
    void insertImageFiles(editor, attachments, content.images, saveSelection(editor));
    return;
  }
  if ("html" in content && insertHtml(content.html)) return;
  execCommand("insertText", event.clipboardData?.getData("text/plain") ?? "");
});

/// Whether the caret is in a list item or a table cell, which hold inline content only.
function caretHoldsInlineOnly(): boolean {
  const caret = rangeWithin(editor)?.startContainer ?? null;
  return ancestorOf(caret, editor, "li", "td", "th") !== null;
}

function insertHtml(html: string): boolean {
  return execCommand("insertHTML", html);
}

function execCommand(command: string, value: string): boolean {
  const exec = (doc as Document & { execCommand?: (c: string, ui: boolean, v: string) => boolean })
    .execCommand;
  return exec?.call(doc, command, false, value) ?? false;
}

// A drop is refused HERE and handled by the host, which is what lets a dropped file become a real
// attachment: the page sees a `File` with no path, while the host gets the path Rust reads the
// bytes from and the row the user removes it by. A dropped picture comes back through
// `insertComposerImage` when the user chooses to show it in the message (Gate 13).
editor.addEventListener("dragover", (event) => event.preventDefault());
editor.addEventListener("drop", (event) => event.preventDefault());

// Keyboard shortcuts. Handled in the bundle so they work identically on every host
// (WKWebView/WebView/WebView2/WebKitGTK) regardless of native menu wiring, and so Tab stays inside
// the editor instead of escaping to the surrounding native form fields.
editor.addEventListener("keydown", (event) => {
  // Mid-composition an IME uses the space bar to commit a candidate; treating that as a marker
  // would swallow the space and mangle the word being typed.
  if (event.isComposing) return;
  // `- ` at the start of a line becomes a bullet, and the space that triggered it is swallowed
  // along with the marker (Outlook's behaviour). `autoformatBulletList` returns false for every
  // other space, which is every space but one.
  if (event.key === " " && !event.metaKey && !event.ctrlKey && !event.altKey) {
    if (autoformatBulletList(editor)) {
      event.preventDefault();
      return;
    }
    // An address the user has just typed becomes a link when the word ends, as in Outlook. The
    // space is inserted after the link rather than left to the engine, which would extend it.
    if (autolinkBeforeCaret(editor, " ")) {
      event.preventDefault();
      return;
    }
  }
  // Enter ends the word too; its default (a new line) then runs as usual.
  if (event.key === "Enter" && !event.metaKey && !event.ctrlKey) {
    autolinkBeforeCaret(editor, null);
  }
  if ((event.metaKey || event.ctrlKey) && !event.altKey && event.key.toLowerCase() === "k") {
    event.preventDefault();
    toolbar.openLink();
    return;
  }
  if ((event.metaKey || event.ctrlKey) && !event.altKey) {
    const command = { b: "bold", i: "italic", u: "underline" }[event.key.toLowerCase()];
    if (command === "bold" || command === "italic" || command === "underline") {
      event.preventDefault();
      applyMark(editor, command);
    }
    return;
  }
  if (event.key !== "Tab") return;
  event.preventDefault();
  // In a list Tab changes the level, which is what every mail client and word processor does and
  // what a table cell's next-cell walk must not pre-empt (a list inside a cell is still a list).
  // Outside both, forward Tab is a literal tab and Shift+Tab does nothing.
  if (indentSelection(editor, event.shiftKey)) return;
  if (moveToAdjacentCell(editor, event.shiftKey)) return;
  if (!event.shiftKey) execCommand("insertText", "\t");
});

editor.addEventListener("mousedown", (event) => {
  if (routeClickBelowSignature(editor, event.target)) event.preventDefault();
});

declare global {
  interface Window {
    addComposerAttachment: (meta: AttachmentMeta) => void;
    addComposerInlineImage: (meta: InlineImageMeta) => void;
    insertComposerImage: (image: string | CapturedImage) => void;
    pasteComposerHtml: (html: unknown) => boolean;
    setComposerQuote: (quote: string | QuoteSeed) => void;
    setComposerQuoteStyle: (style: string) => void;
    completeComposerQuoteAttribution: (attribution: string | Partial<QuoteAttribution>) => void;
    setComposerSignature: (signature: string | SignatureValue | null) => void;
    setSignatureBody: (html: unknown, placeholder?: unknown) => void;
    signatureBody: () => string;
    insertSignatureImage: (image: string | Record<string, unknown>) => void;
    focusComposerBody: () => void;
    setPlainText: (text: unknown) => void;
    setComposerBody: (seed: string | BodySeed) => void;
    useNativeComposerChrome: () => void;
    setComposerTopInset: (cssPx: unknown) => void;
    setComposerLabels: (labels: unknown) => void;
    setComposerHostRequests: (kinds: unknown) => void;
    answerComposerRequest: (id: unknown, answer: unknown) => void;
    composerDocument: () => string;
    composerRevision: () => number;
  }
}

function parsed<T>(value: string | T): T {
  return typeof value === "string" ? (JSON.parse(value) as T) : value;
}

window.addComposerAttachment = (meta) => attachments.add(meta);
window.addComposerInlineImage = (meta) => attachments.addInlineImage(editor, meta);
/// Shows a picture the HOST read in the message body: the "show it in the message" answer to a
/// dropped image. The same path a paste takes, so the two cannot behave differently; the host
/// supplies the bytes because only it can resolve a dropped file to something readable.
window.insertComposerImage = (image) =>
  insertCapturedImage(editor, attachments, parsed<CapturedImage>(image));
/// Pastes formatted text the HOST read off the clipboard, answering whether any went in. Only a host
/// whose WebView hides the clipboard's pictures from the page calls it: it holds the picture back
/// until this says the HTML beside it carried nothing (`pasteContent`).
window.pasteComposerHtml = (html) => {
  const markup = pastedHtml(doc, typeof html === "string" ? html : "", caretHoldsInlineOnly());
  return markup !== null && insertHtml(markup);
};
window.setComposerQuote = (quote) => setComposerQuote(editor, parsed<QuoteSeed>(quote));
window.setComposerQuoteStyle = (style) => setComposerQuoteStyle(editor, style);
window.completeComposerQuoteAttribution = (attribution) =>
  completeComposerQuoteAttribution(editor, parsed<Partial<QuoteAttribution>>(attribution) ?? {});
window.setComposerSignature = (signature) =>
  setComposerSignature(editor, signature == null ? null : parsed<SignatureValue>(signature));
window.setSignatureBody = (html, placeholder) => setSignatureBody(editor, html, placeholder);
window.signatureBody = () => JSON.stringify(signatureBody(editor));
// The HOST decides where the caret opens, never this document. A composer opens in the body or in
// its empty To field (docs/contacts.md §4), and only the host knows which; so the bundle focuses
// nothing on load and waits to be asked. Focusing here as well was invisible on iOS, where DOM
// focus alone neither raises the keyboard nor makes the web view first responder, and stole the
// caret out of To on macOS the moment the bundle finished parsing.
window.focusComposerBody = () => focusComposerBody(editor);
window.setPlainText = (text) => setPlainText(editor, text);
window.setComposerBody = (seed) => setComposerBody(editor, attachments, parsed<BodySeed>(seed) ?? {});
window.useNativeComposerChrome = () => chrome.useNativeComposerChrome();
window.setComposerTopInset = (cssPx) => chrome.setComposerTopInset(cssPx);

/// The requests this host answers with its own UI, `host_requests.ts`; and its answers.
window.setComposerHostRequests = (kinds) => hostRequests.setAnswered(kinds);
window.answerComposerRequest = (id, answer) => hostRequests.answer(id, answer);

window.setComposerLabels = (incoming) => {
  labels = mergeLabels(labels, incoming);
  toolbar.applyLabels(labels);
};

/// Inserts an image at the caret as a self-contained `data:` URI: which is what a signature stores
/// (one file, no side-car blobs to lose) and NOT what the composer's `addComposerInlineImage` does
/// (that mints an attachment id backed by a host blob handle, which a stored signature has no way to
/// keep). On the way out the core rewrites this to a `cid:` part, because Outlook's reader blocks
/// `data:` images.
///
/// The `data:image/` check is the same one `safePreviewUrl` makes: a `data:text/html` here would be
/// an executable document, and the alt text is set as a property so it cannot carry markup into the
/// attribute.
window.insertSignatureImage = (image) => {
  const data = (image ? parsed<Record<string, unknown>>(image) : {}) ?? {};
  const url = String(data.data_url ?? "");
  if (!url.startsWith("data:image/") || safePreviewUrl(url) === "") return;
  const node = doc.createElement("img");
  node.src = url;
  node.alt = String(data.alt_text ?? "");
  if (data.width_px) node.width = Number(data.width_px);
  focusEditor(editor);
  insertAtCaret(editor, node);
};

window.composerRevision = () => revision();

window.composerDocument = () => {
  const blocks = documentBlocks(editor);
  return JSON.stringify({
    blocks,
    attachments: attachments.list(referencedAttachmentIds(blocks)),
  });
};
