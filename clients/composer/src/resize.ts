// Resizing a picture in the message by dragging one of its corners.
//
// In the shared bundle, so one implementation serves all four hosts and the gesture is the same
// with a mouse, a trackpad, a pen and a finger: the grips are driven by Pointer Events, which every
// host's WebView reports for all of them, so there is no separate touch path to drift.
//
// ⚠️ **The grips are not in the editor.** `documentBlocks` and `signatureBody` read the editor's
// own tree, so a chrome element inside it would be emitted as part of the message. The overlay is a
// sibling in `<body>`, placed over the picture from its viewport rect: `position: fixed` and
// `getBoundingClientRect` measure against the same origin, so the two agree with no scroll
// arithmetic of our own.
//
// Only the WIDTH travels. `document.ts` reads it back as `InlineImage::width_px` and the Rust
// renderer emits `<img width>` with the height left to follow, so the aspect ratio is locked and a
// corner is the only thing worth dragging.

import { documentOf, windowOf } from "./dom";

/// The smallest a picture may be dragged to. Below this the grips cover it entirely and there is
/// nothing left to aim at.
const MIN_WIDTH_PX = 24;

/// How far a grip's hit area reaches beyond the corner it marks, matching the largest `::after`
/// inset in `editor.css`. The clip is widened by it, so a grip with nothing to be clipped by keeps
/// its reach.
const GRIP_REACH_PX = 24;

/// The four corners, each as the direction that GROWS the picture when dragged towards it. The
/// west corners grow on a leftward drag even though the picture's left edge cannot move: it is an
/// inline box, anchored where the text flow put it, so "pull away from the middle" is the only
/// reading of a corner that holds for all four.
const CORNERS = {
  nw: { x: -1, y: -1 },
  ne: { x: 1, y: -1 },
  sw: { x: -1, y: 1 },
  se: { x: 1, y: 1 },
} as const;

type CornerName = keyof typeof CORNERS;

/// The picture as it stood when the drag began. Every move is measured from here rather than
/// accumulated, so a pointer that leaves the window and comes back cannot drift the size.
export interface ResizeStart {
  width: number;
  height: number;
  corner: { x: number; y: number };
}

/// The width a drag of (`dx`, `dy`) from `start`'s corner asks for, clamped between the minimum and
/// `max`.
///
/// The travel is PROJECTED onto the corner's outward diagonal rather than read off one axis. The
/// ratio is locked, so a drag along the diagonal grows the picture at the speed of the hand and one
/// across it leaves the size alone, instead of two axes disagreeing about a width and the picture
/// jumping between their answers.
export function draggedWidth(start: ResizeStart, dx: number, dy: number, max: number): number {
  const ceiling = Math.max(max, MIN_WIDTH_PX);
  const diagonal = Math.hypot(start.width, start.height);
  if (diagonal <= 0) return Math.round(Math.min(Math.max(start.width, MIN_WIDTH_PX), ceiling));
  const along = (dx * start.corner.x * start.width + dy * start.corner.y * start.height) / diagonal;
  const width = (start.width * (diagonal + along)) / diagonal;
  return Math.round(Math.min(Math.max(width, MIN_WIDTH_PX), ceiling));
}

/// Writes a dragged width onto the picture, keeping every other size it carries in step.
///
/// A picture the editor inserted travels as a width and nothing else, so the attribute is the whole
/// story. One inside a quoted original or a signature does not: it rides out as raw HTML, with
/// whatever `height` its sender wrote still on the tag, and moving the width without it would
/// squash the picture in the reader's client while looking correct here (`editor.css` forces
/// `height: auto` on every picture in the editor, so nothing on screen would say so).
///
/// `aspect` is the ratio as rendered when the drag began, not one derived from the attributes: a
/// sender's `width`/`height` pair need not match the bytes, and the ratio the user is dragging is
/// the one they can see.
export function applyWidth(image: HTMLImageElement, width: number, aspect: number): void {
  const height = aspect > 0 ? Math.max(1, Math.round(width / aspect)) : 0;
  image.setAttribute("width", String(width));
  if (height > 0 && image.hasAttribute("height")) image.setAttribute("height", String(height));
  // Only a size already declared inline is updated. Adding one would put a rule on the tag the
  // picture never had, and `document.ts` reads the attribute first in any case.
  if (image.style.width) image.style.width = `${width}px`;
  if (height > 0 && image.style.height.endsWith("px")) image.style.height = `${height}px`;
}

/// Installs the grips on `editor`. Every picture in it is resizable, including one in a quoted
/// original or a signature, both of which are editable and both of which carry a width out.
export function installImageResize(editor: HTMLElement): void {
  const doc = documentOf(editor);
  const view = windowOf(editor);

  const overlay = doc.createElement("div");
  overlay.className = "allodia-resize";
  // Chrome, not content: a screen reader announcing a grip would be announcing the mouse's
  // furniture, and the picture's own alt text is what carries it.
  overlay.setAttribute("aria-hidden", "true");
  overlay.hidden = true;

  /// The picture the grips are currently on, and whether a press put them there.
  ///
  /// Hover alone is a mouse's answer and no answer at all on a touch screen, so a press pins the
  /// grips to the picture until something else is pressed. That is also the only route a finger
  /// has: tap the picture, then drag a grip.
  let active: HTMLImageElement | null = null;
  let pinned = false;
  let drag: {
    image: HTMLImageElement;
    pointerId: number;
    start: ResizeStart;
    aspect: number;
    originX: number;
    originY: number;
    max: number;
  } | null = null;

  function hide(): void {
    active = null;
    pinned = false;
    overlay.hidden = true;
  }

  /// Puts the overlay back over its picture, or takes it away when there is no longer one to sit
  /// on: the picture was deleted, the draft was reseeded, or it has been laid out to nothing.
  function place(): void {
    if (!active || !editor.contains(active)) {
      hide();
      return;
    }
    const rect = active.getBoundingClientRect();
    if (rect.width <= 0 || rect.height <= 0) {
      hide();
      return;
    }
    overlay.style.left = `${rect.left}px`;
    overlay.style.top = `${rect.top}px`;
    overlay.style.width = `${rect.width}px`;
    overlay.style.height = `${rect.height}px`;
    overlay.style.clipPath = clipPath(rect);
    overlay.hidden = false;
  }

  /// Clips the overlay to the part of the page its picture is actually visible in.
  ///
  /// The editor scrolls on the desktop hosts and the page scrolls on Android, so a picture can be
  /// scrolled under the toolbar with its grips still shown. Clipping is what a scroll container
  /// does to its own content, and the grips are drawn as though they were in it. The reach is
  /// added back on every side, so an edge that needs no clipping does not lose its grips to one.
  function clipPath(rect: DOMRect): string {
    const box = editor.getBoundingClientRect();
    const inset = (value: number) => `${Math.max(0, value) - GRIP_REACH_PX}px`;
    const top = inset(Math.max(box.top, 0) - rect.top);
    const right = inset(rect.right - Math.min(box.right, view.innerWidth));
    const bottom = inset(rect.bottom - Math.min(box.bottom, view.innerHeight));
    const left = inset(Math.max(box.left, 0) - rect.left);
    return `inset(${top} ${right} ${bottom} ${left})`;
  }

  /// The picture under an event target, when it is one of the editor's own.
  function imageAt(target: EventTarget | null): HTMLImageElement | null {
    const node = target as Element | null;
    if (!node || typeof node.closest !== "function") return null;
    const image = node.closest("img");
    return image && editor.contains(image) ? (image as HTMLImageElement) : null;
  }

  /// The widest the picture can actually be drawn where it sits.
  ///
  /// `editor.css` caps every picture at `max-width: 100%`, so a width past its column would change
  /// the document and nothing on screen, leaving the composer holding a number it is not showing.
  /// Measured from the nearest ancestor with a width of its own, so a picture in a table cell is
  /// bounded by the cell rather than by the editor.
  ///
  /// A tree that measures nothing (a composer laid out behind a hidden tab, say) falls back to the
  /// viewport rather than to the floor: a cap nobody could measure must not collapse the picture to
  /// the smallest size a drag can reach.
  function availableWidth(image: HTMLImageElement): number {
    let host: HTMLElement | null = image.parentElement;
    while (host && host !== editor && host.clientWidth <= 0) host = host.parentElement;
    const box = host ?? editor;
    const style = view.getComputedStyle(box);
    const padding =
      (Number.parseFloat(style.paddingLeft) || 0) + (Number.parseFloat(style.paddingRight) || 0);
    const room = box.clientWidth - padding;
    return room > MIN_WIDTH_PX ? room : Math.max(view.innerWidth, MIN_WIDTH_PX);
  }

  // A move is a drag in progress, or else hover: the mouse's and the trackpad's answer to "which
  // picture is this about". Both are listened for on the DOCUMENT rather than per element. A drag
  // has to survive the pointer leaving the grip, and hover has to survive the opposite, because the
  // grips sit ON the corners: reaching for one takes the pointer off the picture, and a
  // `pointerleave` on the picture would take the grips away as the hand arrived at them.
  doc.addEventListener("pointermove", (event) => {
    if (drag) {
      if (event.pointerId !== drag.pointerId) return;
      event.preventDefault();
      const width = draggedWidth(
        drag.start,
        event.clientX - drag.originX,
        event.clientY - drag.originY,
        drag.max,
      );
      applyWidth(drag.image, width, drag.aspect);
      place();
      return;
    }
    if (event.pointerType !== "mouse") return;
    const image = imageAt(event.target);
    if (image) {
      // Hovering another picture moves the grips to it even when a press pinned them to the last
      // one: the pointer is the answer to "which picture", and a press is only what keeps them
      // there once it leaves.
      active = image;
      place();
      return;
    }
    if (pinned || !active || overlay.contains(event.target as Node)) return;
    const rect = active.getBoundingClientRect();
    const outside =
      event.clientX < rect.left - GRIP_REACH_PX ||
      event.clientX > rect.right + GRIP_REACH_PX ||
      event.clientY < rect.top - GRIP_REACH_PX ||
      event.clientY > rect.bottom + GRIP_REACH_PX;
    if (outside) hide();
  });

  // A press pins the grips to the picture it landed on, and takes them off everything else: a tap
  // on a touch screen, a click with a mouse, and the press on the toolbar that ends either.
  doc.addEventListener("pointerdown", (event) => {
    if (drag || overlay.contains(event.target as Node)) return;
    const image = imageAt(event.target);
    if (!image) {
      hide();
      return;
    }
    active = image;
    pinned = true;
    place();
  });

  for (const name of Object.keys(CORNERS) as CornerName[]) {
    const grip = doc.createElement("div");
    const corner = CORNERS[name];
    grip.className = "grip";
    grip.style.left = corner.x < 0 ? "0" : "100%";
    grip.style.top = corner.y < 0 ? "0" : "100%";
    grip.style.cursor = corner.x === corner.y ? "nwse-resize" : "nesw-resize";
    overlay.appendChild(grip);

    grip.addEventListener("pointerdown", (event) => {
      if (!active) return;
      const rect = active.getBoundingClientRect();
      if (rect.width <= 0 || rect.height <= 0) return;
      // The gesture is ours from here. Without this the host starts a text selection under a mouse
      // and drags the picture itself under a finger, either of which resizes nothing.
      event.preventDefault();
      drag = {
        image: active,
        pointerId: event.pointerId,
        start: { width: rect.width, height: rect.height, corner },
        aspect: rect.width / rect.height,
        originX: event.clientX,
        originY: event.clientY,
        max: availableWidth(active),
      };
      pinned = true;
      // Capture keeps the rest of the page out of the gesture: no hover elsewhere, and nothing
      // under the pointer taking the `pointerup` that ends it. The drag does not depend on it,
      // which is why a refusal is not worth failing over: the events are listened for on the
      // document, so they arrive captured or not.
      try {
        grip.setPointerCapture(event.pointerId);
      } catch {
        // An engine that will not capture this pointer; the drag carries on uncaptured.
      }
    });
  }

  // The move and the end are the DOCUMENT's, not the grip's. A drag that outlives its capture (an
  // engine that refused it, a pointer the host cancels) would otherwise never see its own
  // `pointerup`, and the next pass over the grip would carry on resizing a picture the user let go
  // of.
  const end = (event: PointerEvent) => {
    if (!drag || event.pointerId !== drag.pointerId) return;
    drag = null;
    place();
  };
  doc.addEventListener("pointerup", end);
  doc.addEventListener("pointercancel", end);

  doc.body.appendChild(overlay);

  // `input` is what catches the picture being deleted, and the text around it reflowing under it.
  editor.addEventListener("input", place);
  editor.addEventListener("scroll", place, { passive: true });
  view.addEventListener("scroll", place, { passive: true });
  view.addEventListener("resize", place);
}
