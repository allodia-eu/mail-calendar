// The editor's entry point: resolves the singletons, wires the events, and installs the `window.*`
// seams the four native hosts drive the composer through.
//
// This is the only module that touches the `document`/`window` globals. Everything else takes the
// editor element and derives what it needs from it, which is what lets the commands be tested
// against an isolated DOM.

import { Attachments, type AttachmentMeta, type InlineImageMeta, insertAtCaret, safePreviewUrl } from "./attachments";
import { autoformatBulletList } from "./autoformat";
import { documentBlocks } from "./document";
import { focusEditor } from "./dom";
import { applyMark } from "./format";
import { installNativeChrome } from "./host";
import { DEFAULT_LABELS, type Labels, mergeLabels } from "./labels";
import { indentSelection } from "./lists";
import { setComposerQuote, setComposerQuoteStyle, type QuoteSeed } from "./quote";
import { focusComposerBody, setPlainText } from "./seeds";
import {
  routeClickBelowSignature,
  setComposerSignature,
  setSignatureBody,
  signatureBody,
} from "./signature";
import { moveToAdjacentCell } from "./tables";
import { installToolbar } from "./toolbar";
import type { SignatureValue } from "./types";

const doc = document;
const editor = doc.getElementById("editor") as HTMLElement;
const toolbarRoot = doc.querySelector(".toolbar") as HTMLElement;
const attachments = new Attachments();

let labels: Labels = DEFAULT_LABELS;
const toolbar = installToolbar(editor, toolbarRoot, () => labels);
const chrome = installNativeChrome(editor, toolbarRoot);

// Paste arrives as plain text: the document is a closed schema (marked text runs, lists, tables,
// images), and dropping foreign markup is the strict reading of `docs/composer-security.md` Gate 7.
// Mapping pasted HTML onto the schema is its own piece of work, not an accident of leaving this out.
editor.addEventListener("paste", (event) => {
  event.preventDefault();
  const text = event.clipboardData?.getData("text/plain") ?? "";
  (doc as Document & { execCommand?: (c: string, ui: boolean, v: string) => boolean }).execCommand?.(
    "insertText",
    false,
    text,
  );
});

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
  if (!event.shiftKey) {
    (doc as Document & { execCommand?: (c: string, ui: boolean, v: string) => boolean }).execCommand?.(
      "insertText",
      false,
      "\t",
    );
  }
});

editor.addEventListener("mousedown", (event) => {
  if (routeClickBelowSignature(editor, event.target)) event.preventDefault();
});

declare global {
  interface Window {
    addComposerAttachment: (meta: AttachmentMeta) => void;
    addComposerInlineImage: (meta: InlineImageMeta) => void;
    setComposerQuote: (quote: string | QuoteSeed) => void;
    setComposerQuoteStyle: (style: string) => void;
    setComposerSignature: (signature: string | SignatureValue | null) => void;
    setSignatureBody: (html: unknown, placeholder?: unknown) => void;
    signatureBody: () => string;
    insertSignatureImage: (image: string | Record<string, unknown>) => void;
    focusComposerBody: () => void;
    setPlainText: (text: unknown) => void;
    useNativeComposerChrome: () => void;
    setComposerTopInset: (cssPx: unknown) => void;
    setComposerLabels: (labels: unknown) => void;
    composerDocument: () => string;
  }
}

function parsed<T>(value: string | T): T {
  return typeof value === "string" ? (JSON.parse(value) as T) : value;
}

window.addComposerAttachment = (meta) => attachments.add(meta);
window.addComposerInlineImage = (meta) => attachments.addInlineImage(editor, meta);
window.setComposerQuote = (quote) => setComposerQuote(editor, parsed<QuoteSeed>(quote));
window.setComposerQuoteStyle = (style) => setComposerQuoteStyle(editor, style);
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
window.useNativeComposerChrome = () => chrome.useNativeComposerChrome();
window.setComposerTopInset = (cssPx) => chrome.setComposerTopInset(cssPx);

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

window.composerDocument = () =>
  JSON.stringify({ blocks: documentBlocks(editor), attachments: attachments.list() });
