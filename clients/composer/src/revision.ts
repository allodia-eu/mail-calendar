// How many times the message has changed, for the hosts' autosave.
//
// A draft is stored once the composer has been idle (`docs/drafts.md`), and only this document
// knows that a keystroke happened. There is no channel from this page to the native host, so the
// count is read rather than pushed, which is why it is a counter and not a dirty flag: a host
// samples it and compares against what it last saw, and never has to be told when to clear it.

/// Starts counting changes to `editor`, and answers how many there have been.
///
/// A MutationObserver rather than an `input` listener, because typing is not the only way the
/// message changes: a toolbar command, a pasted picture, a dropped file and a quote restyle all
/// write to the DOM, and several of them write it directly rather than through an editing command.
///
/// The number itself means nothing (it is not an undo depth), and it never goes down. What a host
/// reads it for is the one question "has anything changed since I last looked".
export function installRevisionCounter(editor: HTMLElement): Revision {
  let revision = 0;
  const observer = new (windowOf(editor).MutationObserver)(() => {
    revision += 1;
  });
  observer.observe(editor, {
    childList: true,
    subtree: true,
    characterData: true,
    attributes: true,
  });
  const read = () => revision;
  return Object.assign(read, {
    quietly(change: () => void) {
      // An edit the observer has not delivered yet still counts.
      if (observer.takeRecords().length > 0) revision += 1;
      try {
        change();
      } finally {
        observer.takeRecords();
      }
    },
  });
}

/// How many changes there have been, and a way to make one that is not a change to the message:
/// the page showing its colours for a new scheme (`page.ts`), which sends nothing different.
export type Revision = (() => number) & { quietly(change: () => void): void };

/// The window the editor lives in, rather than the global one, so a test drives an isolated DOM.
function windowOf(node: HTMLElement): Window & typeof globalThis {
  return node.ownerDocument.defaultView as Window & typeof globalThis;
}
