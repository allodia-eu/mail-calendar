// Undo and redo in the message body (`docs/undo.md`).
//
// The history is the editor's, not the engine's. Most of what this editor does to the body is DOM
// surgery rather than an editing command (lists, tables, links, pictures, the marks it rewrites
// after `execCommand`), and none of that is on a WebView's own undo stack; worse, it moves the
// nodes that stack's entries point at, so what an engine's undo does next differs between WebKit
// and Chromium and between one edit and the next. So every change is observed here instead, as the
// mutations it made, and undone by applying them backwards. One implementation, four hosts.
//
// A step is the mutations one change made: one keystroke, one toolbar command, one paste. Typing
// runs into one step until the caret moves, the kind of edit changes or the typing pauses, and a
// drag (a picture's corner) is one step however many moves it took.
//
// Undoing a step restores the nodes it removed, the very same nodes, so a caret saved against them
// is still good, and redoing it is undoing the undo. Nothing is serialised, which is what keeps a
// pasted picture's `data:` URL from being copied into every step after it.

import { windowOf } from "./dom";

/// Typing that pauses for longer than this starts a new step.
export const TYPING_PAUSE_MS = 1000;

/// The most steps kept. The oldest goes first, which is always safe: it is the last one an undo
/// would ever reach.
export const MAX_STEPS = 100;

type Kind = "typing" | "deleting" | "other";

/// Where the selection was, by node, which an undo restores along with the nodes themselves.
interface Caret {
  anchorNode: Node;
  anchorOffset: number;
  focusNode: Node;
  focusOffset: number;
}

interface Step {
  /// The mutations, oldest first. Applying them backwards is the undo.
  records: MutationRecord[];
  /// The selection before the change, restored by an undo, and after it, restored by a redo.
  before: Caret | null;
  after: Caret | null;
  kind: Kind;
  /// When the change last grew, for the typing pause.
  at: number;
  /// The pointer press the change was made in, so a drag stays one step.
  press: number | null;
  /// What the user typed that the editor turned into something else (the space after `- `), which
  /// an undo of this step leaves in place.
  typed?: string;
}

export interface History {
  undo(): boolean;
  redo(): boolean;
  /// Marks the change just made as the editor's reading of `text` the user typed: undoing it puts
  /// back what they typed rather than what the editor made of it.
  typed(text: string): void;
  /// Runs a change the host makes from its own controls (a From change swapping the signature, the
  /// quote style), which is not the user's to undo from the body. The history keeps what it had.
  outside<T>(change: () => T): T;
  /// Runs a change that replaces the body: a draft reopened, a quote seeded. The history starts
  /// over after it, because nothing before it is in the message any more.
  restart<T>(change: () => T): T;
}

/// What a keystroke asks of the history, if anything.
///
/// Ctrl or Cmd with Z is undo, with Shift as well redo, and Ctrl or Cmd with Y redo: every host's
/// own chords between them, accepted on all four as the bundle's other shortcuts are.
export function historyChord(event: KeyboardEvent): "undo" | "redo" | null {
  if (!(event.metaKey || event.ctrlKey) || event.altKey) return null;
  const key = event.key.toLowerCase();
  if (key === "z") return event.shiftKey ? "redo" : "undo";
  if (key === "y" && !event.shiftKey) return "redo";
  return null;
}

/// Starts recording changes to `editor`. `now` is the clock, for a test.
export function installHistory(editor: HTMLElement, now: () => number = Date.now): History {
  const view = windowOf(editor);
  const doc = editor.ownerDocument;
  const undone: Step[] = [];
  const redone: Step[] = [];
  /// The selection as it stood before whatever changes next.
  let caret: Caret | null = null;
  /// The kind of edit an engine is about to make, from its `beforeinput`.
  let pending: Kind | null = null;
  /// Set when the next change may not join the last step.
  let boundary = true;
  let press = 0;
  let pressed = false;
  /// Set while the host changes the body, whose mutations are never a step.
  let quiet = false;

  const observer = new view.MutationObserver((records) => record(records));
  observer.observe(editor, {
    childList: true,
    subtree: true,
    characterData: true,
    characterDataOldValue: true,
    attributes: true,
    attributeOldValue: true,
  });

  function record(records: MutationRecord[]): void {
    if (quiet) return;
    // The editor element's own attributes are chrome (its placeholder and accessible name), and
    // never the message.
    const content = records.filter((r) => !(r.type === "attributes" && r.target === editor));
    if (content.length === 0) return;
    const kind = pending ?? "other";
    pending = null;
    const after = selection();
    const at = now();
    redone.length = 0;
    const last = undone.at(-1);
    if (last && !boundary && joins(last, kind, at)) {
      last.records.push(...content);
      last.after = after;
      last.at = at;
    } else {
      undone.push({ records: content, before: caret, after, kind, at, press: pressed ? press : null });
      if (undone.length > MAX_STEPS) undone.shift();
    }
    boundary = false;
    caret = after;
  }

  function joins(last: Step, kind: Kind, at: number): boolean {
    if (pressed && last.press === press) return true;
    return kind !== "other" && last.kind === kind && at - last.at <= TYPING_PAUSE_MS;
  }

  function flush(): void {
    record(observer.takeRecords());
  }

  function selection(): Caret | null {
    const current = view.getSelection();
    if (!current?.anchorNode || !current.focusNode) return null;
    if (!editor.contains(current.anchorNode) || !editor.contains(current.focusNode)) return null;
    return {
      anchorNode: current.anchorNode,
      anchorOffset: current.anchorOffset,
      focusNode: current.focusNode,
      focusOffset: current.focusOffset,
    };
  }

  function select(target: Caret | null): void {
    const current = view.getSelection();
    if (!target || !current) return;
    if (!editor.contains(target.anchorNode) || !editor.contains(target.focusNode)) return;
    try {
      current.setBaseAndExtent(target.anchorNode, target.anchorOffset, target.focusNode, target.focusOffset);
    } catch {
      // An offset past the end of a node an undo shortened: the content is right, and the caret
      // stays wherever the engine left it.
    }
  }

  /// Takes back the newest step of `from` and files what took it back in `to`, so the opposite
  /// command takes that back in turn. Answers whether there was a step to take back.
  function move(from: Step[], to: Step[]): boolean {
    flush();
    const step = from.pop();
    if (!step) return false;
    const inverse = revert(step.records, observer);
    if (!inverse) {
      // The body no longer matches the step, so nothing in either list can be trusted to.
      undone.length = 0;
      redone.length = 0;
      boundary = true;
      return false;
    }
    const undoing = from === undone;
    select(undoing ? step.before : step.after);
    if (undoing && step.typed) inverse.push(...insertTyped(step.typed));
    const here = selection();
    to.push({
      ...step,
      records: inverse,
      before: undoing ? here : step.before,
      after: undoing ? step.after : here,
      typed: undefined,
      at: now(),
      press: null,
    });
    caret = here;
    boundary = true;
    const inputType = undoing ? "historyUndo" : "historyRedo";
    editor.dispatchEvent(new view.InputEvent("input", { bubbles: true, inputType }));
    return true;
  }

  /// Puts back the text a step's change was made from, at the caret, and answers its mutations.
  function insertTyped(text: string): MutationRecord[] {
    const current = view.getSelection();
    const range = current?.rangeCount ? current.getRangeAt(0) : null;
    if (!current || !range || !editor.contains(range.startContainer)) return [];
    let node = range.startContainer;
    let offset = range.startOffset;
    if (node.nodeType !== doc.TEXT_NODE) {
      const empty = doc.createTextNode("");
      range.insertNode(empty);
      node = empty;
      offset = 0;
    }
    const line = node as Text;
    // A plain space at the end of a line collapses and the caret has nowhere to sit, which is why
    // the engine types a no-break space there; `document.ts` turns it back into a space on send.
    const atEnd = offset >= line.data.length || line.data[offset] === " ";
    line.insertData(offset, text === " " && atEnd ? "\u00a0" : text);
    current.collapse(line, offset + text.length);
    return observer.takeRecords();
  }

  // The engine's own Undo, from a menu or a gesture this bundle does not see as a key, lands here
  // too: its `beforeinput` is cancelled and answered from this history instead.
  editor.addEventListener("beforeinput", (event) => {
    if (event.inputType === "historyUndo" || event.inputType === "historyRedo") {
      event.preventDefault();
      if (event.inputType === "historyUndo") history.undo();
      else history.redo();
      return;
    }
    pending = kindOf(event.inputType);
    // A `beforeinput` whose change never comes (another handler cancelled it) must not label the
    // next change made by script as typing.
    view.setTimeout(() => {
      pending = null;
    }, 0);
  });

  // The caret moving between two keystrokes ends the typing step, so an undo takes back one run of
  // typing rather than every word typed anywhere since the last command.
  doc.addEventListener("selectionchange", () => {
    // A change that moved the caret is filed before the move is read, or the change's own caret
    // would count as the user moving it.
    flush();
    const current = selection();
    const last = undone.at(-1);
    if (!same(current, last?.after ?? null)) boundary = true;
    // A caret gone from the body (into Subject, say) leaves the last one standing: the next change
    // is made where the user comes back to.
    caret = current ?? caret;
  });
  doc.addEventListener("keydown", () => {
    caret = selection() ?? caret;
  }, true);
  doc.addEventListener("pointerdown", () => {
    press += 1;
    pressed = true;
    caret = selection() ?? caret;
  }, true);
  const release = () => {
    pressed = false;
  };
  doc.addEventListener("pointerup", release, true);
  doc.addEventListener("pointercancel", release, true);

  const history: History = {
    undo: () => move(undone, redone),
    redo: () => move(redone, undone),
    typed(text) {
      flush();
      const last = undone.at(-1);
      if (last) last.typed = text;
      boundary = true;
    },
    outside(change) {
      flush();
      quiet = true;
      try {
        return change();
      } finally {
        observer.takeRecords();
        quiet = false;
        caret = selection();
        boundary = true;
      }
    },
    restart(change) {
      flush();
      quiet = true;
      try {
        return change();
      } finally {
        observer.takeRecords();
        quiet = false;
        undone.length = 0;
        redone.length = 0;
        caret = selection();
        boundary = true;
      }
    },
  };
  return history;
}

function kindOf(inputType: string): Kind {
  if (inputType === "insertText" || inputType === "insertCompositionText") return "typing";
  if (inputType.startsWith("delete")) return "deleting";
  return "other";
}

function same(a: Caret | null, b: Caret | null): boolean {
  if (!a || !b) return a === b;
  return (
    a.anchorNode === b.anchorNode &&
    a.anchorOffset === b.anchorOffset &&
    a.focusNode === b.focusNode &&
    a.focusOffset === b.focusOffset
  );
}

/// Applies `records` backwards and answers the mutations that made, which is the step that undoes
/// the undo. `null` when the body has moved on from what they describe, and then nothing changes:
/// what was applied before the mismatch is taken back too, so a half-reverted step never reaches
/// the message.
///
/// The checks are strict on purpose. A node is only removed from the parent the record put it in,
/// and only put back into a gap that still has the neighbour it was taken from; anything else
/// means a change this history never saw (the host swapping the signature, say) has moved the
/// nodes, and guessing would put text somewhere the user never wrote it.
function revert(records: MutationRecord[], observer: MutationObserver): MutationRecord[] | null {
  observer.takeRecords();
  try {
    for (let index = records.length - 1; index >= 0; index -= 1) undoOne(records[index]!);
    return observer.takeRecords();
  } catch {
    const partial = observer.takeRecords();
    try {
      for (let index = partial.length - 1; index >= 0; index -= 1) undoOne(partial[index]!);
    } catch {
      // Taking back our own mutations in order cannot meet a body that moved; nothing to recover.
    }
    observer.takeRecords();
    return null;
  }
}

function undoOne(record: MutationRecord): void {
  if (record.type === "characterData") {
    (record.target as CharacterData).data = record.oldValue ?? "";
    return;
  }
  if (record.type === "attributes") {
    const element = record.target as Element;
    const name = record.attributeName!;
    const namespace = record.attributeNamespace;
    if (record.oldValue === null) element.removeAttributeNS(namespace, name);
    else element.setAttributeNS(namespace, name, record.oldValue);
    return;
  }
  const parent = record.target;
  const added = Array.from(record.addedNodes);
  for (let index = added.length - 1; index >= 0; index -= 1) {
    const node = added[index]!;
    if (node.parentNode !== parent) throw new Error("moved");
    parent.removeChild(node);
  }
  const next = record.nextSibling;
  const previous = record.previousSibling;
  if (record.removedNodes.length === 0) return;
  if (next && next.parentNode !== parent) throw new Error("moved");
  if (!next && previous && parent.lastChild !== previous) throw new Error("moved");
  for (const node of Array.from(record.removedNodes)) {
    if (node.parentNode) throw new Error("moved");
    parent.insertBefore(node, next);
  }
}
