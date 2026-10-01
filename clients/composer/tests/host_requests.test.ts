import { describe, expect, test } from "bun:test";

import { HostRequests } from "../src/host_requests";
import { editLinkThroughHost } from "../src/link_menu";
import { harness } from "./support";

/// A window with one of the three channels a host can offer, recording what was posted.
function hostWindow(channel: "webkit" | "webview2" | "android" | "none") {
  const posted: string[] = [];
  const post = { postMessage: (message: string) => posted.push(message) };
  const win: Record<string, unknown> = {};
  if (channel === "webkit") win.webkit = { messageHandlers: { composerHost: post } };
  if (channel === "webview2") win.chrome = { webview: post };
  if (channel === "android") win.composerHost = post;
  return { win: win as unknown as Window, posted };
}

/// The editor's window, given `channel`, so a request from the link editor reaches `posted`.
function editorWithHost(body: string) {
  const h = harness(body);
  const posted: string[] = [];
  const win = h.editor.ownerDocument.defaultView as unknown as Record<string, unknown>;
  win.webkit = { messageHandlers: { composerHost: { postMessage: (m: string) => posted.push(m) } } };
  const requests = new HostRequests(win as unknown as Window);
  requests.setAnswered(["link"]);
  return { h, posted, requests };
}

function selectWords(h: ReturnType<typeof harness>, selector: string, from: number, to: number) {
  const element = h.caret(selector);
  const doc = element.ownerDocument;
  const range = doc.createRange();
  range.setStart(element.firstChild as never, from);
  range.setEnd(element.firstChild as never, to);
  const selection = doc.defaultView!.getSelection()!;
  selection.removeAllRanges();
  selection.addRange(range as never);
}

describe("the request channel", () => {
  test("asks for nothing until the host names what it answers", async () => {
    const { win, posted } = hostWindow("webkit");
    const requests = new HostRequests(win);
    expect(requests.answers("link")).toBe(false);
    expect(await requests.request("link", { text: "", address: "", removable: false })).toBeNull();
    expect(posted).toEqual([]);
  });

  test("asks for nothing where the WebView offers no channel", () => {
    const requests = new HostRequests(hostWindow("none").win);
    requests.setAnswered(["link"]);
    expect(requests.answers("link")).toBe(false);
  });

  test("ignores a kind outside its vocabulary", () => {
    const requests = new HostRequests(hostWindow("webkit").win);
    requests.setAnswered(["open_url", 7, null]);
    expect(requests.answers("link")).toBe(false);
  });

  for (const channel of ["webkit", "webview2", "android"] as const) {
    test(`posts an externally tagged request and resolves its answer (${channel})`, async () => {
      const { win, posted } = hostWindow(channel);
      const requests = new HostRequests(win);
      requests.setAnswered(["link"]);
      const answer = requests.request("link", { text: "docs", address: "", removable: false });
      expect(posted.map((message) => JSON.parse(message))).toEqual([
        { id: 1, request: { link: { text: "docs", address: "", removable: false } } },
      ]);
      requests.answer(1, { link: { apply: { text: "docs", address: "https://example.com" } } });
      expect(await answer).toEqual({ apply: { text: "docs", address: "https://example.com" } });
    });
  }

  test("an answer of the wrong shape is no answer, and a stray id is ignored", async () => {
    const { win } = hostWindow("webkit");
    const requests = new HostRequests(win);
    requests.setAnswered(["link"]);
    const answer = requests.request("link", { text: "", address: "", removable: false });
    requests.answer(99, { link: "remove" });
    requests.answer(1, { link: { apply: { address: 3 } } });
    expect(await answer).toBeNull();
  });
});

describe("the link editor through the host", () => {
  test("sends the selected words and links them to the answered address", async () => {
    const { h, posted, requests } = editorWithHost("<p id=p>read the agenda first</p>");
    selectWords(h, "#p", 5, 15);
    const done = editLinkThroughHost(h.editor, requests);
    expect(JSON.parse(posted[0]!).request).toEqual({
      link: { text: "the agenda", address: "", removable: false },
    });
    requests.answer(1, { link: { apply: { text: "the agenda", address: "https://example.com" } } });
    await done;
    expect(h.html()).toBe(`<p id="p">read <a href="https://example.com">the agenda</a> first</p>`);
  });

  test("offers removal on a link, and removes it when asked", async () => {
    const { h, posted, requests } = editorWithHost(`<p id=p>see <a id=a href="mailto:a@example.com">Ann</a></p>`);
    selectWords(h, "#a", 1, 2);
    const done = editLinkThroughHost(h.editor, requests);
    expect(JSON.parse(posted[0]!).request).toEqual({
      link: { text: "Ann", address: "a@example.com", removable: true },
    });
    requests.answer(1, { link: "remove" });
    await done;
    expect(h.html()).toBe(`<p id="p">see Ann</p>`);
  });

  test("a cancel, or an address the editor would not send, changes nothing", async () => {
    for (const answer of [{ link: "cancel" }, { link: { apply: { text: "", address: "javascript:alert(1)" } } }]) {
      const { h, requests } = editorWithHost("<p id=p>read the agenda</p>");
      selectWords(h, "#p", 5, 15);
      const done = editLinkThroughHost(h.editor, requests);
      requests.answer(1, answer);
      await done;
      expect(h.html()).toBe(`<p id="p">read the agenda</p>`);
    }
  });
});
