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

export class Attachments {
  private readonly items: DraftAttachment[] = [];

  /// Adds or replaces by id, so a host re-announcing a staged file does not attach it twice.
  private remember(attachment: DraftAttachment): void {
    const index = this.items.findIndex((item) => item.id === attachment.id);
    if (index >= 0) this.items[index] = attachment;
    else this.items.push(attachment);
  }

  list(): DraftAttachment[] {
    return this.items.slice();
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
