// Native-chrome mode (Android): the toolbar is drawn while the message is being written, and gone
// while focus is anywhere else, such as an address field in the native header above the WebView.

import { describe, expect, test } from "bun:test";
import { Window } from "happy-dom";

import { installNativeChrome } from "../src/host";

/// `focusout` re-reads where focus went on the next task, so an assertion waits for it.
const settled = () => new Promise((resolve) => setTimeout(resolve, 0));

function mount() {
  const window = new Window();
  const document = window.document as unknown as Document;
  document.body.innerHTML = `
    <div class="toolbar"><button id="bold">B</button><select id="size"><option>Normal</option><option>Large</option></select></div>
    <main id="editor" contenteditable="true"><p>Half a sentence</p></main>
    <input id="elsewhere">`;
  const editor = document.getElementById("editor") as HTMLElement;
  const toolbar = document.querySelector(".toolbar") as HTMLElement;
  let stopped = 0;
  installNativeChrome(editor, toolbar, () => stopped++).useNativeComposerChrome();
  const writing = () => document.body.classList.contains("composer-writing");
  return { window, document, editor, writing, stopped: () => stopped };
}

/// What the WebView does when the native header takes focus: the page is blurred and
/// `activeElement` stays where it was.
function blurPage(window: Window, document: Document) {
  document.hasFocus = () => false;
  window.dispatchEvent(new window.Event("blur"));
}

describe("the native-chrome toolbar", () => {
  test("is not drawn before anything has focus", () => {
    const { writing } = mount();
    expect(writing()).toBe(false);
  });

  test("is drawn while the message has focus", async () => {
    const { editor, writing } = mount();
    editor.focus();
    await settled();
    expect(writing()).toBe(true);
  });

  test("goes when focus leaves the message", async () => {
    const { document, editor, writing } = mount();
    editor.focus();
    await settled();
    (document.getElementById("elsewhere") as HTMLElement).focus();
    await settled();
    expect(writing()).toBe(false);
  });

  test("goes when the page loses focus to the native header", async () => {
    // The editor still holding `activeElement` is not enough.
    const { window, document, editor, writing } = mount();
    editor.focus();
    await settled();
    blurPage(window, document);
    expect(writing()).toBe(false);
  });

  test("stays while one of its own controls has focus", async () => {
    const { document, writing } = mount();
    (document.getElementById("size") as HTMLElement).focus();
    await settled();
    expect(writing()).toBe(true);
  });

  test("goes when the native header takes focus from one of its controls", async () => {
    // A size picked, then the To field tapped: the toolbar must not stay over the keyboard.
    const { window, document, writing } = mount();
    (document.getElementById("size") as HTMLElement).focus();
    await settled();
    blurPage(window, document);
    expect(writing()).toBe(false);
  });

  test("a pick in one of its selects puts the caret back in the message", async () => {
    const { window, document, editor } = mount();
    const size = document.getElementById("size") as HTMLSelectElement;
    size.focus();
    size.selectedIndex = 1;
    size.dispatchEvent(new window.Event("change", { bubbles: true }) as never);
    await settled();
    expect(document.activeElement).toBe(editor);
  });

  test("closes the toolbar's panels when writing stops, and only then", async () => {
    // An open palette would otherwise come back with the toolbar, over a message the user has
    // since moved on from.
    const { window, document, editor, stopped } = mount();
    editor.focus();
    await settled();
    expect(stopped()).toBe(0);
    blurPage(window, document);
    expect(stopped()).toBe(1);
    blurPage(window, document);
    expect(stopped()).toBe(1);
  });
});
