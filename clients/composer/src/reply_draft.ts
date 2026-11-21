// A drafted reply in the message body (`docs/ai.md` rules 21 to 23).
//
// The host hands a draft over once (`setComposerDraftText`), with the number the core gave it and
// its gaps. It goes in above the signature and the quoted original, replacing what is there, as one
// step of the editor's own history, so one undo takes it out whole and one redo brings it back.
// Once the host has loaded the body, and after every change to it, the editor tells the host what
// the text above those two holds, whether the draft is in it and which of its gaps are left; the
// host hands that to the core, which decides the card's ticks and whether to ask before replacing. Nothing here carries the reply's
// text: a gap is named by its place in the list the host gave.
//
// The draft is untrusted model output: `drafttext.ts` builds it as elements and text nodes, and no
// part of it is ever parsed as markup.

import { caretInto, documentOf, windowOf } from "./dom";
import { draftBlocks } from "./drafttext";
import type { History } from "./history";

/// What a host hands over: `mailcal_app::ai::ReplyDraftScript`'s payload.
export interface DraftSeed {
  serial: number;
  text: string;
  gaps: string[];
  /// False for an editor opened later on the same composition, which only follows the draft
  /// already in its body.
  insert: boolean;
}

/// `mailcal_composer::LeadText`.
export type Lead = "empty" | "draft" | "written";

/// `mailcal_composer::ReplyDraftReport`.
export interface ReplyDraftReport {
  serial: number;
  lead: Lead;
  inserted: boolean;
  left: number[];
  edits: number;
}

/// The draft the editor holds.
interface Held {
  serial: number;
  gaps: string[];
  /// The history's mark on its step, or `null` when it was put there by another editor.
  tag: string | null;
  /// What the text above the signature and the quote reads while it is untouched.
  untouched: string;
}

/// The first direct child that is the signature or the quoted original, which ends the reply's own
/// text. Direct children only: a reply to our own mail carries a second signature inside its quote.
export function leadBoundary(editor: HTMLElement): Element | null {
  return (
    Array.from(editor.children).find(
      (child) => child.classList.contains("allodia-signature") || child.classList.contains("allodia-quote"),
    ) ?? null
  );
}

/// The nodes above the signature and the quote.
function leadNodes(editor: HTMLElement): Node[] {
  const boundary = leadBoundary(editor);
  const nodes: Node[] = [];
  for (let node = editor.firstChild; node && node !== boundary; node = node.nextSibling) nodes.push(node);
  return nodes;
}

/// What `nodes` read as: their text a block to a line, and how many pictures they hold, so a
/// picture added to an untouched draft makes it written.
function reading(nodes: readonly Node[]): string {
  let pictures = 0;
  const lines = nodes.map((node) => {
    if (node.nodeName === "IMG") pictures += 1;
    else if (node.nodeType === 1) pictures += (node as Element).querySelectorAll("img").length;
    return node.textContent ?? "";
  });
  return `${lines.join("\n")}\u0000${pictures}`;
}

function holdsAnything(nodes: readonly Node[]): boolean {
  return nodes.some(
    (node) =>
      (node.textContent ?? "").trim().length > 0 ||
      node.nodeName === "IMG" ||
      (node.nodeType === 1 && (node as Element).querySelector("img") !== null),
  );
}

export interface ReplyDraft {
  /// Takes a draft from the host. Answers whether it was taken: a serial this editor already
  /// holds, or an older one, is not.
  setDraft(seed: unknown): boolean;
  /// Tells the host where the reply stands now, even when nothing changed since it was last told.
  resend(): void;
  /// Runs `load`, which puts the host's content in the body, then tells the host where the reply
  /// stands, even when it reads as before: a fresh reply's quote and signature leave the text above
  /// them as empty as it was, and a report sent before the load may not have reached the core.
  loaded<T>(load: () => T): T;
}

/// Follows `editor`'s reply for `tell`, putting drafts in through `history`.
export function installReplyDraft(
  editor: HTMLElement,
  history: History,
  tell: (report: ReplyDraftReport) => void,
): ReplyDraft {
  let held: Held | null = null;
  let edits = 0;
  let lastReading = reading(leadNodes(editor));
  let told: string | null = null;

  function report(force: boolean): void {
    const nodes = leadNodes(editor);
    const now = reading(nodes);
    if (now !== lastReading) {
      edits += 1;
      lastReading = now;
    }
    const inserted = held !== null && (held.tag === null || history.applied(held.tag));
    const text = now.slice(0, now.lastIndexOf("\u0000"));
    const lead: Lead = !holdsAnything(nodes)
      ? "empty"
      : inserted && held !== null && now === held.untouched
        ? "draft"
        : "written";
    const left = held ? held.gaps.flatMap((gap, at) => (text.includes(gap) ? [at] : [])) : [];
    const next: ReplyDraftReport = { serial: held?.serial ?? 0, lead, inserted, left, edits };
    const key = JSON.stringify(next);
    if (!force && key === told) return;
    told = key;
    tell(next);
  }

  // Every change, however it was made: typing, a toolbar command, a paste, an undo. A
  // MutationObserver's records arrive as a microtask after the change, never on a timer.
  new (windowOf(editor).MutationObserver)(() => report(false)).observe(editor, {
    childList: true,
    subtree: true,
    characterData: true,
  });

  return {
    setDraft(value) {
      const seed = seedOf(value);
      if (!seed || (held !== null && seed.serial <= held.serial)) return false;
      const doc = documentOf(editor);
      const blocks = draftBlocks(doc, seed.text);
      if (seed.insert) {
        const tag = `reply-draft-${seed.serial}`;
        history.step(tag, () => replaceLead(editor, blocks));
        held = { serial: seed.serial, gaps: seed.gaps, tag, untouched: reading(leadNodes(editor)) };
      } else {
        held = { serial: seed.serial, gaps: seed.gaps, tag: null, untouched: reading(blocks) };
      }
      report(false);
      return true;
    },
    resend: () => report(true),
    loaded(load) {
      const result = load();
      report(true);
      return result;
    },
  };
}

/// Puts `blocks` in place of everything above the signature and the quote, and the caret at the
/// end of the last, so the person picks up where the draft stops.
function replaceLead(editor: HTMLElement, blocks: HTMLElement[]): void {
  const boundary = leadBoundary(editor);
  while (editor.firstChild && editor.firstChild !== boundary) editor.removeChild(editor.firstChild);
  for (const block of blocks) editor.insertBefore(block, boundary);
  const last = blocks.at(-1);
  if (last) caretInto(last.lastElementChild?.tagName === "LI" ? last.lastElementChild : last, false);
}

/// The host's seed as JSON or as an object, or `null` when it is not one.
function seedOf(value: unknown): DraftSeed | null {
  let seed = value;
  if (typeof seed === "string") {
    try {
      seed = JSON.parse(seed);
    } catch {
      return null;
    }
  }
  const raw = seed as Partial<Record<keyof DraftSeed, unknown>> | null;
  if (!raw || typeof raw.serial !== "number" || typeof raw.text !== "string") return null;
  const gaps = Array.isArray(raw.gaps) ? raw.gaps.filter((gap): gap is string => typeof gap === "string") : [];
  return { serial: raw.serial, text: raw.text, gaps, insert: raw.insert !== false };
}
