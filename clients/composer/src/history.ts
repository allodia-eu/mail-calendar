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
  /// The attributes and texts the step has already recorded a first value for. A later record of
  /// the same one adds nothing an undo needs, and a drag would otherwise keep one per move.
  seen?: Map<Node, Set<string>>;
  /// What the user typed that the editor turned into something else (the space after `- `), which
  /// an undo of this step leaves in place.
  typed?: string;
}

export interface History {
  undo(): boolean;
  redo(): boolean;
  /// Whether there is a step to take back and one to put back, for a host that draws Undo and Redo
  /// in a menu of its own and has to say whether each can be chosen.
  steps(): { undo: boolean; redo: boolean };
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

/// What a keystroke asks of the history, if anything: the platform's own chords (`docs/undo.md`
/// rule 2). On Apple ⌘Z is undo and ⇧⌘Z redo, and Control is left alone, because Ctrl+Y is
/// Cocoa's yank. Everywhere else Ctrl+Z is undo, and Shift+Ctrl+Z and Ctrl+Y redo.
///
/// The letter is the layout's, falling back to the key's position when the layout has no Latin
/// letter there: on a Cyrillic layout Ctrl+Z types `я`, and is still undo, as it is in every
/// native text field.
export function historyChord(event: KeyboardEvent, apple: boolean): "undo" | "redo" | null {
  const primary = apple ? event.metaKey && !event.ctrlKey : event.ctrlKey && !event.metaKey;
  if (!primary || event.altKey) return null;
  const typed = event.key.toLowerCase();
  const key = /^[a-z]$/.test(typed) ? typed : /^Key([A-Z])$/.exec(event.code)?.[1]?.toLowerCase();
  if (key === "z") return event.shiftKey ? "redo" : "undo";
  if (key === "y" && !event.shiftKey && !apple) return "redo";
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
      last.records.push(...firsts(last, content));
      last.after = after;
      last.at = at;
    } else {
      const step: Step = { records: [], before: caret, after, kind, at, press: pressed ? press : null };
      step.records = firsts(step, content);
      undone.push(step);
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
    const inverse = revert(editor, step.records, observer);
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
      seen: undefined,
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
    // An edit is never part of a drag. A press whose release the page never saw (a `<select>`'s
    // own popup takes it) would otherwise fold every keystroke after it into one step.
    pressed = false;
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
    steps() {
      flush();
      return { undo: undone.length > 0, redo: redone.length > 0 };
    },
    typed(text) {
      flush();
      const last = undone.at(-1);
      if (last) last.typed = text;
      boundary = true;
    },
    outside: (change) => unrecorded(change, false),
    restart: (change) => unrecorded(change, true),
  };

  /// Runs a host's change without filing it as a step; `forget` starts the history over after it.
  function unrecorded<T>(change: () => T, forget: boolean): T {
    flush();
    quiet = true;
    try {
      return change();
    } finally {
      observer.takeRecords();
      quiet = false;
      if (forget) {
        undone.length = 0;
        redone.length = 0;
      }
      caret = selection();
      boundary = true;
    }
  }
  return history;
}

/// `records` less the ones `step` already holds an older value for. Undoing runs backwards, so of
/// several changes to one attribute or one text only the first one's old value is ever the result.
function firsts(step: Step, records: MutationRecord[]): MutationRecord[] {
  const seen = (step.seen ??= new Map());
  return records.filter((record) => {
    if (record.type === "childList") return true;
    const name = record.type === "attributes" ? `@${record.attributeName}` : "#text";
    const names = seen.get(record.target) ?? new Set<string>();
    seen.set(record.target, names);
    if (names.has(name)) return false;
    names.add(name);
    return true;
  });
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
/// nodes, and guessing would put text somewhere the user never wrote it. For the same reason every
/// node the step touched must be back in the body afterwards, unless the step itself put it there:
/// a text the host has since taken out would otherwise take the undo invisibly.
function revert(editor: HTMLElement, records: MutationRecord[], observer: MutationObserver): MutationRecord[] | null {
  observer.takeRecords();
  try {
    for (let index = records.length - 1; index >= 0; index -= 1) undoOne(records[index]!);
    const added = records.flatMap((record) => Array.from(record.addedNodes));
    for (const record of records) {
      const target = record.target;
      if (!editor.contains(target) && !added.some((node) => node.contains(target))) throw new Error("moved");
    }
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
