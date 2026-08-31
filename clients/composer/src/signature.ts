// The sender's signature region, and the body-only seams the Settings signature editor uses.

import { caretInto, documentOf, focusEditor } from "./dom";
import type { Block, SignatureValue } from "./types";

/// **This message's** signature region; a DIRECT child of the editor, never a descendant.
///
/// The scoping is load-bearing, not tidiness. Our own outgoing mail wraps the sender's signature in
/// `.allodia-signature`, so replying to a message we sent puts a second one inside the quoted
/// original's `.aq-body`. A plain `querySelector(".allodia-signature")` finds the QUOTED one first,
/// it comes earlier in document order once the quote is seeded; and every swap then rewrites the
/// original author's signature instead of the reply's, destroying quoted content the user never
/// touched. The composer's own region is always a direct child and a quoted one is always inside
/// `.allodia-quote`, so "direct child" separates them exactly.
export function composerSignature(editor: HTMLElement): HTMLElement | null {
  return (
    (Array.from(editor.children).find((child) =>
      child.classList.contains("allodia-signature"),
    ) as HTMLElement | undefined) ?? null
  );
}

/// The `Signature` block emitted to the shared composer: the region's current HTML (the user may
/// have edited it before sending) and the plain-text rendering that rides alongside it into the
/// `text/plain` part. The core sanitises both halves again on submit.
export function signatureBlock(container: HTMLElement): Block {
  return {
    Signature: {
      body_html: container.innerHTML,
      body_plain: container.dataset.signaturePlain || "",
    },
  };
}

/// Inserts, replaces, or removes the signature region. `null` removes it ("None" in the picker, or
/// an account with no signature).
///
/// Three rules make this safe to call at any time, which is what auto-swap on a From change needs:
///
///  * It touches ONLY the signature region. The user's typed text, their trimming of the quote and
///    the caret are all left where they are; a swap must not eat what they wrote. (Edits made to
///    the signature *itself* are replaced, which is what Outlook does too: the block belongs to the
///    chosen signature.)
///  * Placement is decided once, on first insert: above the quoted original when there is one (so a
///    reply reads message → signature → original, the Outlook default), otherwise at the end of the
///    body. A later swap reuses that position rather than re-deciding, so the signature does not hop
///    around as the user changes sender.
///  * `body_html` is assigned as markup: that is the point of a rich signature: and it is inert by
///    then: the core sanitises a signature when it is stored AND again on submit
///    (`docs/composer-security.md`, Gate 10).
export function setComposerSignature(editor: HTMLElement, signature: SignatureValue | null): void {
  const doc = documentOf(editor);
  let container = composerSignature(editor);

  if (!signature?.body_html) {
    container?.remove();
    return;
  }

  if (!container) {
    container = doc.createElement("div");
    container.className = "allodia-signature";
    const quote = editor.querySelector(".allodia-quote");
    if (quote) editor.insertBefore(container, quote);
    else editor.appendChild(container);
    // A new message opens with an empty editor, so the signature would land as its FIRST element,
    // and `focusComposerBody` puts the caret in the first element, which would drop the user inside
    // their own signature with nowhere above it to write. Give them the empty line the composer
    // would otherwise have had.
    if (!container.previousElementSibling) {
      const lead = doc.createElement("p");
      lead.appendChild(doc.createElement("br"));
      editor.insertBefore(lead, container);
    }
  }
  container.innerHTML = signature.body_html;
  container.dataset.signaturePlain = signature.body_plain || "";
}

/// Routes a click in the editor's empty area **below everything** to the MESSAGE, never the
/// signature.
///
/// This is not a nicety. The signature is the last block in the document, so `contenteditable`'s
/// default; put the caret at the end of the nearest block; drops it inside the signature for every
/// click in the large blank area under it. The user then types their message *into* their signature,
/// and the next `setComposerSignature` (a From change, or the picker) replaces that region and their
/// text goes with it. Silent data loss, from the most natural click in the composer.
///
/// Scoped to "a signature is present": with none, reply/forward behaviour is exactly as it was.
export function routeClickBelowSignature(editor: HTMLElement, target: EventTarget | null): boolean {
  // A click that landed on real content (text, the quote, the signature itself) has a more specific
  // target; leave those alone, so editing the signature is still possible.
  if (target !== editor) return false;
  // Scoped for the same reason as `setComposerSignature`: a quoted original may carry its sender's
  // signature, and routing the caret relative to *that* would drop it in the quote.
  const signature = composerSignature(editor);
  if (!signature) return false;

  let anchor = signature.previousElementSibling;
  if (!anchor) {
    const lead = documentOf(editor).createElement("p");
    lead.appendChild(documentOf(editor).createElement("br"));
    editor.insertBefore(lead, signature);
    anchor = lead;
  }
  caretInto(anchor, false);
  focusEditor(editor);
  return true;
}

// --- Signature authoring (the Settings editor) ---
//
// The Settings signature editor is this same bundle in a body-only mode: the whole editor *is* the
// signature, with no recipients, no subject, and no quote. It gets its own seams rather than reusing
// the composer's, because what it round-trips is a bare HTML fragment (what a `StoredSignature`
// holds); not a block document.

/// Loads an existing signature for editing. Markup, deliberately: a stored signature is sanitised by
/// the core when it is saved, so what arrives here is already inert.
///
/// `placeholder` re-labels the empty editor: the bundle's default says "Write your message", which
/// is a lie in the Settings signature editor. Optional, so a host that doesn't pass one keeps the
/// composer wording rather than showing nothing.
export function setSignatureBody(editor: HTMLElement, html: unknown, placeholder?: unknown): void {
  editor.innerHTML = typeof html === "string" ? html : "";
  if (typeof placeholder === "string" && placeholder) {
    editor.dataset.placeholder = placeholder;
    editor.setAttribute("aria-label", placeholder);
  }
  focusEditor(editor);
}

/// The authored signature: the HTML to store, and the plain-text rendering that becomes the
/// `text/plain` half of every message it is seeded into. `innerText` (not `textContent`) is what
/// respects the line structure the user can see: `textContent` would run a three-line signature
/// into one.
export function signatureBody(editor: HTMLElement): SignatureValue {
  return { body_html: editor.innerHTML, body_plain: editor.innerText };
}
