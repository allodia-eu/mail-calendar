import { describe, expect, test } from "bun:test";

import { autoformatBulletList } from "../src/autoformat";
import { historyChord, installHistory, MAX_STEPS, TYPING_PAUSE_MS } from "../src/history";
import { autolinkBeforeCaret } from "../src/links";
import { indentSelection } from "../src/lists";
import { harness } from "./support";

/// A MutationObserver delivers in a microtask, and each keystroke is a task of its own.
const settled = () => new Promise((resolve) => setTimeout(resolve, 0));

/// An editor with a history on a clock the test moves.
function editing(body: string) {
  const h = harness(body);
  const clock = { now: 0 };
  const history = installHistory(h.editor, () => clock.now);
  const view = h.editor.ownerDocument.defaultView!;

  function caretAt(node: Node, offset: number): void {
    const range = h.editor.ownerDocument.createRange();
    range.setStart(node as never, offset);
    range.collapse(true);
    const selection = view.getSelection()!;
    selection.removeAllRanges();
    selection.addRange(range as never);
  }

  /// What an engine does for one keystroke: announce it, then change the text at the caret.
  async function key(inputType: string, change: (text: Text) => void): Promise<void> {
    const event = new view.InputEvent("beforeinput", { inputType, bubbles: true, cancelable: true });
    h.editor.dispatchEvent(event as never);
    const selection = view.getSelection()!;
    const at = selection.anchorNode as unknown as Node;
    const text = (at.nodeType === 3 ? at : at.lastChild) as Text;
    change(text);
    // Moved through the range rather than the selection, which happy-dom answers with a
    // `selectionchange` on the spot; an engine sends it a task later, after the change is filed.
    selection.getRangeAt(0).setStart(text as never, text.data.length);
    selection.getRangeAt(0).collapse(true);
    await settled();
    clock.now += 10;
  }

  const type = async (word: string) => {
    for (const letter of word) await key("insertText", (text) => {
      text.data += letter;
    });
  };
  const backspace = () => key("deleteContentBackward", (text) => {
    text.data = text.data.slice(0, -1);
  });

  return { ...h, history, clock, view, caretAt, type, backspace };
}

describe("undo and redo", () => {
  test("take back a change the editor made to the document, and put it back", async () => {
    const e = editing("<ul><li id=a>a</li><li id=b>b</li></ul>");
    e.caret("#b");
    indentSelection(e.editor, false);
    await settled();
    const indented = e.html();
    expect(e.history.undo()).toBe(true);
    expect(e.html()).toBe('<ul><li id="a">a</li><li id="b">b</li></ul>');
    expect(e.history.redo()).toBe(true);
    expect(e.html()).toBe(indented);
  });

  test("restore the very nodes a change removed, so nothing else pointing at them breaks", async () => {
    const e = editing("<p id=p>kept</p>");
    const paragraph = e.editor.querySelector("#p")!;
    paragraph.remove();
    await settled();
    e.history.undo();
    expect(e.editor.querySelector("#p")).toBe(paragraph);
  });

  test("answer false when there is nothing to take back", () => {
    const e = editing("<p>untouched</p>");
    expect(e.history.undo()).toBe(false);
    expect(e.history.redo()).toBe(false);
  });

  test("say what there is to take back and to put back, for a host's own menu", async () => {
    const e = editing("<p id=p>one</p>");
    expect(e.history.steps()).toEqual({ undo: false, redo: false });
    // Not yet delivered by the observer: a menu opened straight after a change still offers it.
    e.editor.querySelector("#p")!.append(" two");
    expect(e.history.steps()).toEqual({ undo: true, redo: false });
    e.history.undo();
    expect(e.history.steps()).toEqual({ undo: false, redo: true });
  });

  test("tell a watching host what there is to take back and to put back, once per change", async () => {
    const e = editing("<p id=p>one</p>");
    const told: unknown[] = [];
    e.history.watch((steps) => told.push(steps));
    expect(told).toEqual([{ undo: false, redo: false }]);
    e.editor.querySelector("#p")!.append(" two");
    await settled();
    e.editor.querySelector("#p")!.append(" three");
    await settled();
    expect(told).toEqual([
      { undo: false, redo: false },
      { undo: true, redo: false },
    ]);
    e.history.undo();
    e.history.undo();
    expect(told.at(-1)).toEqual({ undo: false, redo: true });
    e.history.restart(() => e.editor.querySelector("#p")!.append(" four"));
    expect(told.at(-1)).toEqual({ undo: false, redo: false });
  });

  test("a new change after an undo leaves nothing to redo", async () => {
    const e = editing("<p id=p>one</p>");
    e.editor.querySelector("#p")!.append(" two");
    await settled();
    e.history.undo();
    e.editor.querySelector("#p")!.append(" three");
    await settled();
    expect(e.history.redo()).toBe(false);
    expect(e.html()).toBe('<p id="p">one three</p>');
  });

  test("put the caret back where it was before the change", async () => {
    const e = editing("<p id=p>Dear Sam,</p>");
    const text = e.editor.querySelector("#p")!.firstChild!;
    e.caretAt(text, 4);
    await e.type("est");
    e.history.undo();
    expect(e.html()).toBe('<p id="p">Dear Sam,</p>');
    const selection = e.view.getSelection()!;
    expect(selection.anchorNode as unknown).toBe(text);
    expect(selection.anchorOffset).toBe(4);
  });

  test("tell the page the body changed, as the engine's own undo does", async () => {
    const e = editing("<p id=p>one</p>");
    const seen: string[] = [];
    e.editor.addEventListener("input", (event) => seen.push((event as InputEvent).inputType));
    e.editor.querySelector("#p")!.append(" two");
    await settled();
    e.history.undo();
    e.history.redo();
    expect(seen).toEqual(["historyUndo", "historyRedo"]);
  });

  test("keep the newest steps when there are more than it holds", async () => {
    const e = editing("<p id=p></p>");
    const paragraph = e.editor.querySelector("#p")!;
    for (let index = 0; index < MAX_STEPS + 5; index += 1) {
      paragraph.append(String(index % 10));
      await settled();
    }
    let undone = 0;
    while (e.history.undo()) undone += 1;
    expect(undone).toBe(MAX_STEPS);
    expect(paragraph.textContent).toBe("01234");
  });
});

describe("one step", () => {
  test("is one run of typing", async () => {
    const e = editing("<p id=p>x</p>");
    e.caretEnd("#p");
    await e.type(" marks the spot");
    e.history.undo();
    expect(e.html()).toBe('<p id="p">x</p>');
  });

  test("ends where the typing paused", async () => {
    const e = editing("<p id=p>x</p>");
    e.caretEnd("#p");
    await e.type(" marks");
    e.clock.now += TYPING_PAUSE_MS + 1;
    await e.type(" the spot");
    e.history.undo();
    expect(e.html()).toBe('<p id="p">x marks</p>');
  });

  test("ends where the caret moved", async () => {
    const e = editing("<p id=p>x</p><p id=q>y</p>");
    e.caretEnd("#p");
    await e.type("1");
    e.caretEnd("#q");
    e.editor.ownerDocument.dispatchEvent(new e.view.Event("selectionchange") as never);
    await e.type("2");
    e.history.undo();
    expect(e.html()).toBe('<p id="p">x1</p><p id="q">y</p>');
  });

  test("ends at a keystroke, even after a press whose release the page never saw", async () => {
    const e = editing("<p id=p>x</p>");
    e.caretEnd("#p");
    // A `<select>`'s own popup takes the release, so the page sees the press alone.
    e.editor.ownerDocument.dispatchEvent(new e.view.Event("pointerdown") as never);
    e.editor.querySelector("#p")!.setAttribute("style", "font-size: 20px");
    await settled();
    await e.type("abc");
    e.history.undo();
    expect(e.html()).toBe('<p id="p" style="font-size: 20px">x</p>');
  });

  test("ends where typing turns into deleting", async () => {
    const e = editing("<p id=p>x</p>");
    e.caretEnd("#p");
    await e.type("abc");
    await e.backspace();
    e.history.undo();
    expect(e.html()).toBe('<p id="p">xabc</p>');
  });

  test("is a whole drag, however many moves it took", async () => {
    const e = editing('<p><img id=i width="100"></p>');
    const image = e.editor.querySelector("#i")!;
    const doc = e.editor.ownerDocument;
    doc.dispatchEvent(new e.view.Event("pointerdown") as never);
    for (const width of ["120", "140", "160"]) {
      image.setAttribute("width", width);
      await settled();
      e.clock.now += TYPING_PAUSE_MS * 2;
    }
    doc.dispatchEvent(new e.view.Event("pointerup") as never);
    e.history.undo();
    expect(image.getAttribute("width")).toBe("100");
  });

  test("keeps one old value per attribute however long the drag, and still undoes it", async () => {
    const e = editing('<p><img id=i width="100"></p>');
    const image = e.editor.querySelector("#i")!;
    const doc = e.editor.ownerDocument;
    doc.dispatchEvent(new e.view.Event("pointerdown") as never);
    for (let width = 101; width <= 400; width += 1) {
      image.setAttribute("width", String(width));
      await settled();
    }
    doc.dispatchEvent(new e.view.Event("pointerup") as never);
    e.history.undo();
    expect(image.getAttribute("width")).toBe("100");
    e.history.redo();
    expect(image.getAttribute("width")).toBe("400");
  });

  test("is not the editor's placeholder or accessible name changing", async () => {
    const e = editing("<p id=p>one</p>");
    e.editor.querySelector("#p")!.append(" two");
    await settled();
    e.editor.dataset.placeholder = "Message";
    e.editor.setAttribute("aria-label", "Message");
    await settled();
    e.history.undo();
    expect(e.html()).toBe('<p id="p">one</p>');
  });
});

describe("what the editor made of what was typed", () => {
  test("a `- ` bullet undoes to the text typed, space included", async () => {
    const e = editing("<p id=p>-</p>");
    e.caretAt(e.editor.querySelector("#p")!.firstChild!, 1);
    e.editor.dispatchEvent(new e.view.KeyboardEvent("keydown", { key: " ", bubbles: true }) as never);
    expect(autoformatBulletList(e.editor)).toBe(true);
    e.history.typed(" ");
    expect(e.html()).toBe("<ul><li><br></li></ul>");
    e.history.undo();
    expect(e.editor.textContent).toBe("-\u00a0");
    expect(e.editor.querySelector("ul")).toBeNull();
    e.history.redo();
    expect(e.html()).toBe("<ul><li><br></li></ul>");
  });

  test("an address made a link undoes to the address and the space after it", async () => {
    const e = editing("<p id=p>see www.example.com</p>");
    const text = e.editor.querySelector("#p")!.firstChild!;
    e.caretAt(text, "see www.example.com".length);
    e.editor.dispatchEvent(new e.view.KeyboardEvent("keydown", { key: " ", bubbles: true }) as never);
    expect(autolinkBeforeCaret(e.editor, " ")).toBe(true);
    e.history.typed(" ");
    e.history.undo();
    expect(e.editor.querySelector("a")).toBeNull();
    expect(e.editor.textContent).toBe("see www.example.com\u00a0");
  });
});

describe("the host's own changes", () => {
  test("are not undone from the body, and the user's steps around them still are", async () => {
    const e = editing('<p id=p>one</p><div class="allodia-signature" id=s>Old</div>');
    e.editor.querySelector("#p")!.append(" two");
    await settled();
    e.history.outside(() => {
      e.editor.querySelector("#s")!.textContent = "New";
    });
    expect(e.history.undo()).toBe(true);
    expect(e.html()).toBe('<p id="p">one</p><div class="allodia-signature" id="s">New</div>');
    expect(e.history.undo()).toBe(false);
  });

  test("a body seeded afresh leaves nothing to undo", async () => {
    const e = editing("<p id=p>one</p>");
    e.editor.querySelector("#p")!.append(" two");
    await settled();
    e.history.restart(() => {
      e.editor.innerHTML = "<p>Reopened draft</p>";
    });
    expect(e.history.undo()).toBe(false);
    expect(e.html()).toBe("<p>Reopened draft</p>");
  });

  test("a step the host's change has overtaken is refused whole, never half applied", async () => {
    const e = editing("<p id=p>one</p><p id=q>two</p><div id=s>Old</div>");
    // One step that takes a paragraph out from in front of the signature and adds to another.
    e.editor.querySelector("#q")!.remove();
    e.editor.querySelector("#p")!.append("!");
    await settled();
    e.history.outside(() => {
      const signature = e.editor.ownerDocument.createElement("div");
      signature.textContent = "New";
      e.editor.querySelector("#s")!.replaceWith(signature);
    });
    const before = e.html();
    // The paragraph's place was in front of a signature that is no longer in the message, so the
    // step cannot be taken back exactly, and the "!" it also added stays rather than going alone.
    expect(e.history.undo()).toBe(false);
    expect(e.html()).toBe(before);
    expect(e.history.undo()).toBe(false);
  });

  test("a step in text the host has since taken out is refused, never taken invisibly", async () => {
    const e = editing('<p id=p>one</p><div class="allodia-signature" id=s>Old</div>');
    (e.editor.querySelector("#s")!.firstChild as Text).data = "Older";
    await settled();
    e.history.outside(() => {
      e.editor.querySelector("#s")!.textContent = "New";
    });
    const before = e.html();
    expect(e.history.undo()).toBe(false);
    expect(e.html()).toBe(before);
  });
});

describe("the engine's own undo", () => {
  test("is answered from this history, and the engine's is never run", async () => {
    const e = editing("<p id=p>one</p>");
    e.editor.querySelector("#p")!.append(" two");
    await settled();
    const event = new e.view.InputEvent("beforeinput", {
      inputType: "historyUndo",
      bubbles: true,
      cancelable: true,
    });
    e.editor.dispatchEvent(event as never);
    expect(event.defaultPrevented).toBe(true);
    expect(e.html()).toBe('<p id="p">one</p>');
  });
});

describe("the chords", () => {
  const chord = (key: string, modifiers: Partial<KeyboardEvent> = {}, apple = false) =>
    historyChord(
      { key, code: "", metaKey: false, ctrlKey: false, altKey: false, shiftKey: false, ...modifiers } as KeyboardEvent,
      apple,
    );

  test("are Ctrl+Z, with Shift or as Ctrl+Y for redo, off Apple", () => {
    expect(chord("z", { ctrlKey: true })).toBe("undo");
    expect(chord("Z", { ctrlKey: true, shiftKey: true })).toBe("redo");
    expect(chord("y", { ctrlKey: true })).toBe("redo");
    expect(chord("z", { metaKey: true })).toBeNull();
  });

  test("are ⌘Z and ⇧⌘Z on Apple, where Ctrl+Y is the text system's yank", () => {
    expect(chord("z", { metaKey: true }, true)).toBe("undo");
    expect(chord("Z", { metaKey: true, shiftKey: true }, true)).toBe("redo");
    expect(chord("y", { ctrlKey: true }, true)).toBeNull();
    expect(chord("y", { metaKey: true }, true)).toBeNull();
    expect(chord("z", { ctrlKey: true }, true)).toBeNull();
  });

  test("find Z by its position on a layout with no Latin letter there", () => {
    expect(chord("я", { ctrlKey: true, code: "KeyZ" })).toBe("undo");
    // A Latin layout's own letter wins over the position: AZERTY has W where QWERTY has Z.
    expect(chord("w", { ctrlKey: true, code: "KeyZ" })).toBeNull();
  });

  test("leave a plain Z, an Alt chord and every other key alone", () => {
    expect(chord("z")).toBeNull();
    expect(chord("z", { ctrlKey: true, altKey: true })).toBeNull();
    expect(chord("y", { ctrlKey: true, shiftKey: true })).toBeNull();
    expect(chord("b", { ctrlKey: true })).toBeNull();
  });
});

describe("a host change that moves the caret", () => {
  test("is still not a step, even when the caret move is reported in the middle of it", async () => {
    const e = editing('<p id=p>one</p><div class="allodia-signature" id=s>Old</div>');
    e.editor.querySelector("#p")!.append(" two");
    await settled();
    e.history.outside(() => {
      e.editor.querySelector("#s")!.textContent = "New";
      // happy-dom reports a selection change on the spot, as an engine never does.
      e.caretEnd("#s");
      e.editor.querySelector("#s")!.append("!");
    });
    e.history.undo();
    expect(e.html()).toBe('<p id="p">one</p><div class="allodia-signature" id="s">New!</div>');
  });
});
