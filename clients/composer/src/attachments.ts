// The draft's attachment manifest, and inserting an inline image the host has staged.

import { documentOf, focusEditor } from "./dom";
import type { DraftAttachment } from "./types";

/// Metadata a host passes for a file it has staged behind a blob handle.
export interface AttachmentMeta {
  id: string;
  blob: string;
  file_name: string;
  media_type: string;
  size?: number | null;
}

/// An inline image adds the CID its `<img src="cid:…">` will reference, plus a local preview URL
/// the editor can actually display before the bytes ever become a MIME part.
export interface InlineImageMeta extends AttachmentMeta {
  cid: string;
  alt_text?: string;
  width_px?: number | null;
  preview_url?: string;
}

/// The bytes and metadata for a picture the editor captured itself; it mints the id and the
/// content id, because there is no host staging step to mint them.
export interface CapturedImageMeta {
  data_url: string;
  file_name: string;
  media_type: string;
}

export class Attachments {
  private readonly items: DraftAttachment[] = [];
  /// Sequence for the ids and content ids minted for captured pictures. Monotonic for the life of
  /// the document, so removing a picture and pasting another never reuses a retired id.
  private captured = 0;

  /// Adds or replaces by id, so a host re-announcing a staged file does not attach it twice.
  private remember(attachment: DraftAttachment): void {
    const index = this.items.findIndex((item) => item.id === attachment.id);
    if (index >= 0) this.items[index] = attachment;
    else this.items.push(attachment);
  }

  /// The manifest, with every inline image the body no longer references dropped.
  ///
  /// An inline attachment is only ever valid alongside the `<img>` that points at it: Rust rejects
  /// a document carrying an unreferenced one, so a picture the user pasted and then deleted (or
  /// one that ended up inside a quoted original, which travels as HTML rather than as a document
  /// node) would otherwise make the message unsendable. `referenced` is the id set
  /// `documentBlocks` actually emitted, so the manifest and the body cannot disagree.
  ///
  /// Regular attachments are never pruned: they stand on their own, with nothing in the body to
  /// reference them.
  list(referenced?: ReadonlySet<string>): DraftAttachment[] {
    if (!referenced) return this.items.slice();
    return this.items.filter(
      (item) => item.disposition === "Attachment" || referenced.has(item.id),
    );
  }

  add(meta: AttachmentMeta): void {
    this.remember({
      id: String(meta.id),
      blob: String(meta.blob),
      file_name: String(meta.file_name),
      media_type: String(meta.media_type),
      size: meta.size == null ? null : Number(meta.size),
      disposition: "Attachment",
    });
  }

  addInlineImage(editor: HTMLElement, meta: InlineImageMeta): void {
    this.remember({
      id: String(meta.id),
      blob: String(meta.blob),
      file_name: String(meta.file_name),
      media_type: String(meta.media_type),
      size: meta.size == null ? null : Number(meta.size),
      disposition: { Inline: { cid: String(meta.cid) } },
    });

    const image = documentOf(editor).createElement("img");
    image.dataset.attachmentId = String(meta.id);
    image.alt = String(meta.alt_text || "");
    if (meta.width_px) image.width = Number(meta.width_px);
    image.src = safePreviewUrl(meta.preview_url);
    focusEditor(editor);
    insertAtCaret(editor, image);
  }

  /// Records a picture whose bytes travel in the document, returning the id the `<img>` must
  /// carry. No `blob`: there is no staged file, and Rust requires exactly one of the two.
  ///
  /// `size` is deliberately left null. It exists to catch a host handing bytes for the wrong blob,
  /// and there is no second channel here to disagree with.
  addCapturedImage(meta: CapturedImageMeta): string {
    const seq = this.captured;
    this.captured += 1;
    const id = `captured-${seq}`;
    this.remember({
      id,
      file_name: meta.file_name || `image-${seq}${extensionFor(meta.media_type)}`,
      media_type: meta.media_type || mediaTypeOf(meta.data_url) || "image/png",
      size: null,
      disposition: { Inline: { cid: mintContentId(seq) } },
      data_url: meta.data_url,
    });
    return id;
  }
}

/// A content id for a captured picture, unique within the message.
///
/// The clock alone is not enough: several pictures pasted in one batch are minted inside the same
/// millisecond, so the sequence carries the uniqueness and the clock only keeps two drafts of the
/// same message apart. None of the bytes a `Content-ID` header forbids can appear here.
function mintContentId(seq: number): string {
  return `img${seq}.${Date.now()}@allodia.local`;
}

/// The media type a `data:` URI declares, or an empty string when it declares none.
export function mediaTypeOf(dataUrl: string): string {
  const head = dataUrl.slice(0, dataUrl.indexOf(","));
  return head.startsWith("data:") ? head.slice("data:".length).split(";")[0] ?? "" : "";
}

/// A file-name extension for a picture whose source had no name (a pasted screenshot). Cosmetic:
/// the part is addressed by its content id, but a reader listing parts should not show a blank.
function extensionFor(mediaType: string): string {
  const subtype = mediaType.split("/")[1] ?? "";
  if (subtype === "jpeg") return ".jpg";
  if (subtype === "svg+xml") return ".svg";
  return /^[a-z0-9]+$/.test(subtype) ? `.${subtype}` : ".img";
}

/// Only a local image preview may reach an `<img src>`. A `data:text/html` is an executable
/// document, and a remote URL would be a network fetch the composer is not allowed to make.
export function safePreviewUrl(url: unknown): string {
  return typeof url === "string" && (url.startsWith("data:image/") || url.startsWith("blob:"))
    ? url
    : "";
}

/// Drops `node` in at the caret, or at the end of the body when there is no caret in the editor.
export function insertAtCaret(editor: HTMLElement, node: Node): void {
  const selection = documentOf(editor).defaultView?.getSelection();
  if (!selection || selection.rangeCount === 0 || !editor.contains(selection.getRangeAt(0).startContainer)) {
    editor.appendChild(node);
    return;
  }
  const range = selection.getRangeAt(0);
  range.deleteContents();
  range.insertNode(node);
  range.setStartAfter(node);
  range.collapse(true);
  selection.removeAllRanges();
  selection.addRange(range);
}
