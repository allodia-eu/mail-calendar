// Pictures the editor captures itself: a pasted screenshot, and the "show it in the message"
// answer to a dropped image file.
//
// Neither has a staged file behind it, so neither has a host blob handle to reference. The bytes
// ride in the document as a base64 `data:` URI instead (`mailcal_composer::DraftAttachment`'s
// `data_url`), and the core decodes them into the `cid:` MIME part the sent body points at. That
// keeps the whole path in shared code: one implementation of "a pasted picture arrives inline",
// identical on all four hosts, rather than four host bridges that can drift.
//
// The document never carries anything else this way: Rust holds an in-document attachment to an
// inline `data:image/…`, and a regular file attachment always stays a host-staged path so a large
// PDF is streamed from disk rather than base64'd through a JavaScript string.

import { type Attachments, insertAtCaret, mediaTypeOf } from "./attachments";
import {
  caretInto,
  documentOf,
  focusEditor,
  rangeWithin,
  restoreSelection,
  type SavedSelection,
} from "./dom";

/// The largest picture the editor will carry in a document.
///
/// Base64 inflates by a third and the whole document crosses the FFI as one string, so an
/// unbounded paste (a camera original, a print-resolution scan) would cost several times the
/// file's size in memory on the way out. Well above any screenshot, well below what a mail server
/// would accept. A file over it is dropped rather than truncated; see `docs/composer-security.md`
/// under "Known gaps".
const MAX_CAPTURED_BYTES = 20 * 1024 * 1024;

/// A picture ready to go into the document: its bytes as a `data:` URI, plus the metadata the
/// outgoing MIME part needs.
export interface CapturedImage {
  data_url: string;
  file_name?: string;
  media_type?: string;
  alt_text?: string;
  width_px?: number | null;
}

/// The picture formats a message body may carry: the raster set every platform decodes natively,
/// and the same closed list the core sniffs a dropped file against
/// (`mailcal_app::composer_image::raster_media_type`). SVG is deliberately absent, on the clipboard
/// exactly as on a drop: it is script-capable, and nothing script-capable belongs behind an `<img>`
/// the core turns into a `cid:` part.
const SHOWABLE_TYPES = ["image/png", "image/jpeg", "image/gif", "image/webp"];

/// Whether `mediaType` names one of the formats above, parameters and casing aside.
export function isShowableImage(mediaType: string): boolean {
  return SHOWABLE_TYPES.includes(mediaType.trim().toLowerCase().split(";")[0] ?? "");
}

/// The image files a paste or a drop carried, in the order the platform listed them.
///
/// Anything that is not a showable picture is left alone: a pasted `.docx` is not something the
/// body can show, and neither is an SVG.
///
/// ⚠️ **`files` is a fallback, never a second source.** The two channels describe the same
/// pictures, and `getAsFile()` mints a fresh `File` on every call, so reading both and deduplicating
/// by object identity let one Ctrl+V through twice: two `cid:` parts and two `<img>` tags for a
/// single pasted screenshot. `items` is read first rather than dropped because some engines expose
/// a clipboard image through it alone, with no file list beside it.
export function imageFilesFrom(transfer: DataTransfer | null | undefined): File[] {
  if (!transfer) return [];
  const fromItems = showable(
    Array.from(transfer.items ?? [])
      .filter((item) => item.kind === "file")
      .map((item) => item.getAsFile()),
  );
  return fromItems.length > 0 ? fromItems : showable(Array.from(transfer.files ?? []));
}

/// The pictures among `candidates`, in order, dropping what a message body may not carry.
function showable(candidates: (File | null)[]): File[] {
  return candidates.filter((file): file is File => file !== null && isShowableImage(file.type));
}

/// Reads one image file into a `data:` URI. Resolves to `null` for anything that is not a
/// readable picture of a showable format within the size cap, so a caller can simply skip it.
export function readImageFile(file: File): Promise<CapturedImage | null> {
  if (!isShowableImage(file.type) || file.size > MAX_CAPTURED_BYTES) {
    return Promise.resolve(null);
  }
  return new Promise((resolve) => {
    const reader = new FileReader();
    reader.onerror = () => resolve(null);
    reader.onload = () => {
      const url = typeof reader.result === "string" ? reader.result : "";
      resolve(
        isShowableImage(mediaTypeOf(url))
          ? { data_url: url, file_name: file.name, media_type: file.type }
          : null,
      );
    };
    reader.readAsDataURL(file);
  });
}

/// Inserts a captured picture at the caret and records the inline attachment that backs it.
///
/// `saved` restores a selection captured before an asynchronous read, so the picture lands where
/// the user pasted rather than wherever the caret drifted to while the file was being read.
/// Returns whether anything was inserted.
export function insertCapturedImage(
  editor: HTMLElement,
  attachments: Attachments,
  image: CapturedImage,
  saved: SavedSelection | null = null,
): boolean {
  const url = String(image.data_url ?? "");
  // The same check the core repeats on submit: a `data:text/html` here would be an executable
  // document, and only a picture of a showable format may end up behind the `cid:` an `<img>`
  // points at. An SVG fails it too, which is why the test is the closed list and not `image/`.
  if (!url.startsWith("data:") || !isShowableImage(mediaTypeOf(url))) return false;

  const id = attachments.addCapturedImage({
    data_url: url,
    file_name: typeof image.file_name === "string" ? image.file_name : "",
    media_type: typeof image.media_type === "string" ? image.media_type : "",
  });

  const node = documentOf(editor).createElement("img");
  node.dataset.attachmentId = id;
  node.src = url;
  // Set as a property, never as markup, so a file name cannot carry an attribute into the tag.
  node.alt = typeof image.alt_text === "string" ? image.alt_text : "";
  if (image.width_px) node.width = Number(image.width_px);
  focusEditor(editor);
  restoreSelection(editor, saved);
  caretIntoMessage(editor);
  insertAtCaret(editor, node);
  return true;
}

/// Puts the caret where a picture that arrived without one belongs: the end of the message, above
/// the signature and above the quoted original.
///
/// A drop is the case: the user was dragging rather than typing, so the editor may hold no caret at
/// all, and appending to the end of the document would put the picture *below* a reply's signature
/// and its quoted original. A no-op whenever there is already a caret in the editor, which is every
/// paste and every drop onto a composer being written in.
function caretIntoMessage(editor: HTMLElement): void {
  if (rangeWithin(editor)) return;
  const message = Array.from(editor.children).filter(
    (child) =>
      !child.classList.contains("allodia-signature") && !child.classList.contains("allodia-quote"),
  );
  const last = message[message.length - 1];
  if (last) caretInto(last, false);
}

/// Reads every image in `files` and inserts them, in order, where the user pasted them.
///
/// Sequential rather than concurrent: the reads resolve in whatever order the platform finishes
/// them, and two pictures pasted together must arrive in the order they were listed. `saved` is
/// the caret as it stood before the first (asynchronous) read; only the first insertion restores
/// it, because each insertion leaves the caret after the picture it added, which is exactly where
/// the next one belongs.
export async function insertImageFiles(
  editor: HTMLElement,
  attachments: Attachments,
  files: File[],
  saved: SavedSelection | null = null,
): Promise<number> {
  let inserted = 0;
  for (const file of files) {
    const image = await readImageFile(file);
    if (image && insertCapturedImage(editor, attachments, image, inserted === 0 ? saved : null)) {
      inserted += 1;
    }
  }
  return inserted;
}
