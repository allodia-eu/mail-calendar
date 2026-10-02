// Asking the host for what only it can do well: a dialog over the window, rather than a panel
// inside this WebView's own frame, which a small window makes too small to use.
//
// One channel, `composerHost`, reached through whatever the platform's WebView offers. Its
// vocabulary is `mailcal_composer::HostRequestKind`, and every host parses a message there and
// nowhere else, so a request this file can send is one the core has a type for. A host answers by
// calling `answerComposerRequest` with the request's id. Until a host names the requests it
// answers (`setComposerHostRequests`), the editor asks for nothing and draws its own UI.

/// `mailcal_composer::LinkRequest`.
export interface LinkRequest {
  text: string;
  address: string;
  removable: boolean;
}

/// `mailcal_composer::LinkAnswer`: externally tagged, its unit variants bare strings.
export type LinkAnswer = { apply: { text: string; address: string } } | "remove" | "cancel";

/// Each request kind, with what it carries and what it is answered with.
interface Vocabulary {
  link: { request: LinkRequest; answer: LinkAnswer };
}

export type HostRequestKind = keyof Vocabulary;

const KINDS: readonly HostRequestKind[] = ["link"];

type Post = (message: string) => void;

/// The channel this WebView offers, or `null` when it offers none.
function channelOf(win: Window): Post | null {
  const scope = win as unknown as {
    webkit?: { messageHandlers?: { composerHost?: { postMessage(message: string): void } } };
    chrome?: { webview?: { postMessage(message: string): void } };
    composerHost?: { postMessage(message: string): void };
  };
  // A `WKScriptMessageHandler` (Apple) or a script-message handler (WebKitGTK).
  const handler = scope.webkit?.messageHandlers?.composerHost;
  if (handler) return (message) => handler.postMessage(message);
  // WebView2's web message, present only when the host enabled it.
  const webview = scope.chrome?.webview;
  if (webview) return (message) => webview.postMessage(message);
  // Android's injected object.
  const injected = scope.composerHost;
  if (injected) return (message) => injected.postMessage(message);
  return null;
}

export class HostRequests {
  private readonly answered = new Set<HostRequestKind>();
  private readonly pending = new Map<number, (answer: unknown) => void>();
  private nextId = 1;

  constructor(private readonly win: Window) {}

  /// The host's list of what it answers. Anything not in this editor's vocabulary is ignored.
  setAnswered(kinds: unknown): void {
    this.answered.clear();
    if (!Array.isArray(kinds)) return;
    for (const kind of kinds) {
      if (KINDS.includes(kind as HostRequestKind)) this.answered.add(kind as HostRequestKind);
    }
  }

  /// Whether `kind` should go to the host rather than be drawn here.
  answers(kind: HostRequestKind): boolean {
    return this.answered.has(kind) && channelOf(this.win) !== null;
  }

  /// Sends one request and resolves with the host's answer, or `null` when it cannot be sent or
  /// the answer is not one this kind takes.
  request<K extends HostRequestKind>(
    kind: K,
    payload: Vocabulary[K]["request"],
  ): Promise<Vocabulary[K]["answer"] | null> {
    const post = channelOf(this.win);
    if (!post || !this.answered.has(kind)) return Promise.resolve(null);
    const id = this.nextId++;
    return new Promise((resolve) => {
      this.pending.set(id, (answer) => resolve(answerOf(kind, answer)));
      post(JSON.stringify({ id, request: { [kind]: payload } }));
    });
  }

  /// The host's answer to request `id`. An id nothing is waiting for is ignored.
  answer(id: unknown, value: unknown): void {
    const key = Number(id);
    const resolve = this.pending.get(key);
    if (!resolve) return;
    this.pending.delete(key);
    resolve(value);
  }
}

function answerOf<K extends HostRequestKind>(kind: K, value: unknown): Vocabulary[K]["answer"] | null {
  const answer = (value as Record<string, unknown> | null)?.[kind];
  if (kind === "link") {
    if (answer === "remove" || answer === "cancel") return answer;
    const apply = (answer as { apply?: { text?: unknown; address?: unknown } } | undefined)?.apply;
    if (apply && typeof apply.text === "string" && typeof apply.address === "string") {
      return { apply: { text: apply.text, address: apply.address } };
    }
  }
  return null;
}
