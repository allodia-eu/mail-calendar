import { beforeEach, describe, expect, test } from "bun:test";
import { Window } from "happy-dom";

import { Attachments } from "../src/attachments";
import { documentBlocks } from "../src/document";
import { insertCapturedImage } from "../src/images";
import { applyWidth, draggedWidth, installImageResize, type ResizeStart } from "../src/resize";
import { signatureBody } from "../src/signature";
import { harness } from "./support";

/// A 1×1 transparent PNG, the shape a pasted screenshot arrives in.
const PNG =
  "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg==";

/// A 400×300 picture, dragged from whichever corner the test is about.
function start(corner: { x: number; y: number }): ResizeStart {
  return { width: 400, height: 300, corner };
}

const NW = { x: -1, y: -1 };
const NE = { x: 1, y: -1 };
const SE = { x: 1, y: 1 };
const ROOM = 10000;

describe("a corner drag", () => {
  test("grows the picture by the distance travelled along its diagonal", () => {
    // 400×300 has a 500 diagonal, so a (40, 30) pull away from the bottom-right corner is 50 along
    // it: a tenth longer, and a tenth wider.
    expect(draggedWidth(start(SE), 40, 30, ROOM)).toBe(440);
    expect(draggedWidth(start(SE), -40, -30, ROOM)).toBe(360);
  });

  test("grows on a pull away from the middle, from every corner", () => {
    // The picture's left edge cannot move: it is an inline box, anchored where the text flow put
    // it. So a west corner has to grow on a LEFTWARD drag, or two of the four handles would shrink
    // the picture when pulled outwards.
    expect(draggedWidth(start(NW), -40, -30, ROOM)).toBe(440);
    expect(draggedWidth(start(NE), 40, -30, ROOM)).toBe(440);
  });

  test("leaves the size alone when the pointer moves across the diagonal", () => {
    // The ratio is locked, so only travel along the diagonal is a size. Without the projection the
    // two axes each answer with a different width and the picture jumps between them.
    expect(draggedWidth(start(SE), 30, -40, ROOM)).toBe(400);
  });

  test("stops at the width the column can actually show", () => {
    // `editor.css` caps a picture at `max-width: 100%`. Past that the document would carry a number
    // the composer is not showing, so the drag stops where the picture does.
    expect(draggedWidth(start(SE), 5000, 3750, 620)).toBe(620);
  });

  test("stops before the picture is too small to grab", () => {
    expect(draggedWidth(start(SE), -5000, -3750, ROOM)).toBe(24);
    // A column narrower than the floor must not invert the clamp into a zero-width picture.
    expect(draggedWidth(start(SE), -5000, -3750, 10)).toBe(24);
  });

  test("answers with the starting width for a picture that has no size yet", () => {
    expect(draggedWidth({ width: 0, height: 0, corner: SE }, 40, 30, ROOM)).toBe(24);
  });
});

describe("the width a drag writes", () => {
  let window: Window;
  let image: HTMLImageElement;

  function mount(markup: string) {
    window = new Window();
    window.document.body.innerHTML = markup;
    image = window.document.querySelector("img") as unknown as HTMLImageElement;
  }

  test("is the attribute `document.ts` reads back as `width_px`", () => {
    mount("<img>");
    applyWidth(image, 320, 4 / 3);
    expect(image.getAttribute("width")).toBe("320");
  });

  test("takes a quoted picture's own height with it", () => {
    // A picture inside a quoted original or a signature travels as raw HTML, with whatever `height`
    // its sender wrote still on the tag. Moving the width without it squashes the picture in the
    // reader's client while looking right here, because the editor forces `height: auto`.
    mount('<img width="400" height="300">');
    applyWidth(image, 200, 400 / 300);
    expect(image.getAttribute("width")).toBe("200");
    expect(image.getAttribute("height")).toBe("150");
  });

  test("leaves a picture with no height of its own without one", () => {
    // The editor's own pictures carry a width and nothing else: the Rust renderer emits `<img
    // width>` and lets the height follow, so writing one here would pin what should follow.
    mount("<img>");
    applyWidth(image, 320, 4 / 3);
    expect(image.hasAttribute("height")).toBe(false);
  });

  test("keeps an inline size in step but adds none", () => {
    mount('<img style="width: 400px; height: 300px">');
    applyWidth(image, 200, 400 / 300);
    expect(image.style.width).toBe("200px");
    expect(image.style.height).toBe("150px");

    mount("<img>");
    applyWidth(image, 200, 4 / 3);
    expect(image.getAttribute("style")).toBeNull();
  });

  test("is emitted as the picture's `width_px`", () => {
    const { editor, caret } = harness("<p>Look: </p>");
    caret("p");
    const attachments = new Attachments();
    insertCapturedImage(editor, attachments, { data_url: PNG });
    applyWidth(editor.querySelector("img") as HTMLImageElement, 260, 4 / 3);

    const [block] = documentBlocks(editor);
    const inline = (block as { Paragraph: { content: { Image?: { width_px?: number } }[] } })
      .Paragraph.content.find((item) => "Image" in item);
    expect(inline?.Image?.width_px).toBe(260);
  });
});

describe("the grips", () => {
  let window: Window;
  let editor: HTMLElement;
  let image: HTMLImageElement;

  /// happy-dom lays nothing out, so the picture is told what rectangle it occupies: 400×300 at the
  /// origin, inside an editor of the same box.
  function rect(width: number, height: number) {
    return { x: 0, y: 0, top: 0, left: 0, right: width, bottom: height, width, height } as DOMRect;
  }

  function press(target: EventTarget, type: string, x: number, y: number, pointerType = "mouse") {
    target.dispatchEvent(
      new window.PointerEvent(type, {
        bubbles: true,
        clientX: x,
        clientY: y,
        pointerId: 1,
        pointerType,
      }) as never,
    );
  }

  function grip(index: number): HTMLElement {
    const grips = window.document.querySelectorAll(".allodia-resize .grip");
    return grips[index] as unknown as HTMLElement;
  }

  beforeEach(() => {
    window = new Window();
    window.document.body.innerHTML = '<main id="editor" contenteditable="true"><p><img></p></main>';
    editor = window.document.getElementById("editor") as unknown as HTMLElement;
    image = editor.querySelector("img") as unknown as HTMLImageElement;
    image.getBoundingClientRect = () => rect(400, 300);
    editor.getBoundingClientRect = () => rect(620, 800);
    installImageResize(editor);
  });

  test("are not part of the message", () => {
    // `documentBlocks` and `signatureBody` read the editor's own tree, so chrome inside it would be
    // sent. The overlay is a sibling in `<body>`, which is also what keeps it out of a signature
    // the Settings editor round-trips through this same document.
    expect(editor.querySelector(".allodia-resize")).toBeNull();
    expect(window.document.body.querySelector(".allodia-resize")).not.toBeNull();
    expect(signatureBody(editor).body_html).toBe("<p><img></p>");
  });

  test("appear on the picture the pointer is over, and go when it leaves", () => {
    const overlay = window.document.querySelector(".allodia-resize") as unknown as HTMLElement;
    expect(overlay.hidden).toBe(true);

    press(image, "pointermove", 200, 150);
    expect(overlay.hidden).toBe(false);
    expect(overlay.style.width).toBe("400px");

    press(editor, "pointermove", 600, 700);
    expect(overlay.hidden).toBe(true);
  });

  test("stay on a picture a finger tapped, which has no hover to keep them there", () => {
    const overlay = window.document.querySelector(".allodia-resize") as unknown as HTMLElement;
    press(image, "pointerdown", 200, 150, "touch");
    expect(overlay.hidden).toBe(false);

    // The gesture that follows is a drag on a grip, so the pointer moving away must not take them
    // off the picture the way it does for a mouse.
    press(editor, "pointermove", 600, 700, "touch");
    expect(overlay.hidden).toBe(false);

    press(editor, "pointerdown", 600, 700, "touch");
    expect(overlay.hidden).toBe(true);
  });

  test("resize the picture as a finger drags one, not only as a mouse does", () => {
    press(image, "pointerdown", 200, 150, "touch");
    const corner = grip(3);
    press(corner, "pointerdown", 400, 300, "touch");
    press(corner, "pointermove", 440, 330, "touch");
    press(corner, "pointerup", 440, 330, "touch");

    expect(image.getAttribute("width")).toBe("440");
  });

  test("measure every move from where the drag began, so a pointer cannot drift the size", () => {
    press(image, "pointermove", 200, 150);
    const corner = grip(3);
    press(corner, "pointerdown", 400, 300);
    press(corner, "pointermove", 440, 330);
    press(corner, "pointermove", 480, 360);
    press(corner, "pointerup", 480, 360);

    expect(image.getAttribute("width")).toBe("480");
  });

  test("follow the mouse to another picture even after a click pinned them to one", () => {
    // The pointer is the answer to "which picture is this about"; a press is only what keeps the
    // grips there once it leaves. Without this, clicking a picture to put the caret beside it would
    // make every other picture in the message unresizable until the user clicked away.
    const second = window.document.createElement("img") as unknown as HTMLImageElement;
    second.getBoundingClientRect = () => rect(200, 100);
    editor.querySelector("p")!.appendChild(second as never);
    const overlay = window.document.querySelector(".allodia-resize") as unknown as HTMLElement;

    press(image, "pointerdown", 200, 150);
    expect(overlay.style.width).toBe("400px");

    press(second, "pointermove", 100, 50);
    expect(overlay.style.width).toBe("200px");
  });

  test("let go of the picture even when the pointer was never captured", () => {
    // A capture the engine refuses, or one the host cancels, sends the rest of the gesture to
    // whatever is under the pointer instead of to the grip. The drag has to end all the same, or
    // the next pass over the grip carries on resizing a picture the user let go of.
    press(image, "pointermove", 200, 150);
    const corner = grip(3);
    press(corner, "pointerdown", 400, 300);
    press(corner, "pointermove", 440, 330);
    press(editor, "pointerup", 440, 330);
    press(editor, "pointermove", 600, 700);

    expect(image.getAttribute("width")).toBe("440");
  });

  test("go when the picture the user was resizing is deleted", () => {
    const overlay = window.document.querySelector(".allodia-resize") as unknown as HTMLElement;
    press(image, "pointerdown", 200, 150);
    expect(overlay.hidden).toBe(false);

    image.remove();
    editor.dispatchEvent(new window.Event("input", { bubbles: true }) as never);
    expect(overlay.hidden).toBe(true);
  });
});
