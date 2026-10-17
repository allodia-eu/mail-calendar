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
    <div class="toolbar"><button id="bold">B</button><select id="size"><option>Normal</option></select></div>
    <main id="editor" contenteditable="true"><p>Half a sentence</p></main>
    <input id="elsewhere">`;
  const editor = document.getElementById("editor") as HTMLElement;
  const toolbar = document.querySelector(".toolbar") as HTMLElement;
  installNativeChrome(editor, toolbar).useNativeComposerChrome();
  const writing = () => document.body.classList.contains("composer-writing");
  return { window, document, editor, writing };
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
    // The host's address field taking focus blurs the WebView's page and leaves `activeElement`
    // on the editor, so the editor holding it is not enough.
    const { window, document, editor, writing } = mount();
    editor.focus();
    await settled();
    document.hasFocus = () => false;
    window.dispatchEvent(new window.Event("blur"));
    expect(writing()).toBe(false);
  });

  test("stays while one of its own controls has focus", async () => {
    // A `select`'s picker blurs the page while it is open; the control under it must stay.
    const { window, document, writing } = mount();
    (document.getElementById("size") as HTMLElement).focus();
    await settled();
    document.hasFocus = () => false;
    window.dispatchEvent(new window.Event("blur"));
    expect(writing()).toBe(true);
  });
});
