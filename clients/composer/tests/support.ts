// Test scaffolding: a fresh editor in its own happy-dom window per test.
//
// Every editing command takes the editor element and derives its document and window from it, so
// nothing here registers globals; each test owns an isolated DOM and they cannot leak into one
// another.

import { Window } from "happy-dom";

export interface Harness {
  editor: HTMLElement;
  /// The editor's inner HTML with the whitespace between tags collapsed, so an assertion reads as
  /// the structure under test rather than as the indentation of the fixture that produced it.
  html(): string;
  /// Puts the caret inside the first element matching `selector`.
  ///
  /// Generic so the narrow element types the commands require (a cell, a list item) are inferred
  /// from the parameter the result is passed to, rather than cast at every call site. The cast
  /// itself is unavoidable and is made once, in `find`: happy-dom models its own DOM interface
  /// rather than lib.dom's, so none of its elements are assignable to the types `src/` is written
  /// against (happy-dom#1227).
  caret<T extends HTMLElement = HTMLElement>(selector: string): T;
  /// Selects from the start of `from` to the end of `to`, for the multi-item commands.
  select(from: string, to: string): void;
  /// Puts the caret at the END of the first element matching `selector`.
  caretEnd<T extends HTMLElement = HTMLElement>(selector: string): T;
  /// The element the caret currently sits in, or `null` when there is no selection.
  caretHost(): HTMLElement | null;
}

export function harness(body: string): Harness {
  const window = new Window();
  const document = window.document;
  document.body.innerHTML = `<main id="editor" contenteditable="true">${body}</main>`;
  const editor = document.getElementById("editor") as unknown as HTMLElement;

  const find = <T extends HTMLElement = HTMLElement>(selector: string): T => {
    const node = editor.querySelector(selector);
    if (!node) throw new Error(`no element matches ${selector}`);
    return node as unknown as T;
  };

  return {
    editor,
    html: () => editor.innerHTML.replace(/>\s+</g, "><").trim(),
    caret<T extends HTMLElement = HTMLElement>(selector: string): T {
      const target = find<T>(selector);
      const range = document.createRange();
      range.selectNodeContents(target as never);
      range.collapse(true);
      const selection = window.getSelection()!;
      selection.removeAllRanges();
      selection.addRange(range);
      return target;
    },
    caretEnd<T extends HTMLElement = HTMLElement>(selector: string): T {
      const target = find<T>(selector);
      const range = document.createRange();
      range.selectNodeContents(target as never);
      range.collapse(false);
      const selection = window.getSelection()!;
      selection.removeAllRanges();
      selection.addRange(range);
      return target;
    },
    caretHost() {
      const selection = window.getSelection();
      if (!selection || selection.rangeCount === 0) return null;
      const node = selection.getRangeAt(0).startContainer as unknown as Node;
      const element = node.nodeType === 1 ? (node as unknown as Element) : node.parentElement;
      return (element as HTMLElement | null) ?? null;
    },
    select(from, to) {
      const start = find(from);
      const end = find(to);
      const range = document.createRange();
      range.setStart(start as never, 0);
      range.setEnd(end as never, end.childNodes.length);
      const selection = window.getSelection()!;
      selection.removeAllRanges();
      selection.addRange(range);
    },
  };
}
