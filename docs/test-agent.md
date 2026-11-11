# The test agent

Every desktop client can answer a test, a script or a coding agent from inside the running app:
find a control, act on it, wait for the screen to change, all in one request each and without a
pixel coordinate. This doc is the protocol every client's agent speaks, so one client
([`scripts/dev/agent.py`](../scripts/dev/agent.py)) and one conformance suite
([`scripts/dev/agent_conformance.py`](../scripts/dev/agent_conformance.py)) drive them all.

Each agent is its own code, because each walks its own toolkit; what is shared is this wire.

## Rules

1. **Debug builds only, and only when asked.** The agent is compiled out of a release build and
   starts only when `MAILCAL_TEST_AGENT` names an endpoint at launch. No installed copy answers.
2. **It answers the person who started the app and nobody else.** A Unix socket is created mode
   0600 in a directory only its owner can enter; a named pipe takes the default ACL of its creator.
3. **It does what a person could do, and refuses what a person could not.** A disabled control, a
   read-only field, a list position past the end and a row that does nothing are refusals with a
   code, never a silent success. An agent that acts where a person cannot passes tests the product
   would fail.
4. **It never guesses.** A filter that matches nothing names what was near; `press` and `fill`
   refuse two matches. An unknown verb or argument is a refusal, so a misspelt filter cannot match
   everything.
5. **Waiting is a request, never a sleep.** `wait` returns the moment its filter matches (or
   stops matching), and `settle` returns when the app and the core have both been quiet.
6. **The accessibility tree stays the judge of what assistive technology gets.** The agent reads
   the toolkit's own objects, which is why it is fast; a suite that asserts on what a screen
   reader says keeps asking the platform's accessibility API (`linux_ui_atspi.py`, `uia.ps1`,
   `macos-ax.swift`, idb).

## Endpoints

| Platform | `MAILCAL_TEST_AGENT` names | Where the agent lives |
|---|---|---|
| Linux | a Unix socket's path (`linux_agent_socket` in [`linux_session.sh`](../scripts/dev/linux_session.sh) picks one per checkout) | [`clients/linux/src/agent.rs`](../clients/linux/src/agent.rs) |
| Windows | a named pipe's name | [`TestAgent.cs`](../clients/windows/Mailcal/Services/TestAgent.cs) |
| macOS, iOS Simulator | a Unix socket's path (`apple_agent_socket` in [`apple_agent.sh`](../scripts/dev/apple_agent.sh) picks one per checkout and platform; the simulator shares the Mac's filesystem) | [`TestAgent.swift`](../clients/apple/Packages/MailcalKit/Sources/MailcalUI/TestAgent/TestAgent.swift) |

## Wire

UTF-8, one JSON object per line in each direction, one reply per request, in order. One client at
a time; when it goes, the next connection is served.

A request names its verb, and every other field is an argument:

```json
{"verb": "wait", "text": "Message sent", "timeoutMs": 5000}
```

A reply is `{"ok": true, …}` with the verb's fields, or a refusal:

```json
{"ok": false, "code": "timeout", "error": "nothing matched within 5000 ms", "near": ["[41] GtkLabel \"Sending…\" label"]}
```

A caller branches on `code`, never on `error`, which is for a person to read.

## Elements

An element is named by a **handle**, `h`: a positive integer the agent hands out, stable for as
long as the element lives and never reused. A handle whose element has gone is refused with
`gone`; one never handed out, with `no-element`.

Every element is described the same way (`query`, `wait`, `read`):

| Field | Always | Meaning |
|---|---|---|
| `h` | yes | The handle. |
| `type` | yes | The toolkit's class (`GtkToggleButton`, `Button`, `NSButton`). |
| `role` | yes | The accessible role, spelled as the platform spells it. |
| `text` | no | What a person reads on it: labels, a title, a value, as a list. |
| `id` | no | The identifier a developer gave it (GTK widget name, `AutomationId`, `accessibilityIdentifier`), which no person reads. |
| `tooltip`, `icon`, `value` | no | Its tooltip, the named icon it shows, its current value. |
| `bounds` | when laid out | `[x, y, width, height]` in logical pixels of its window. |
| `showing` | yes | Drawn on screen now. |
| `enabled` | yes | A person could act on it. |
| `focused`, `active`, `selected`, `expanded`, `readOnly` | when they apply | Its state. |

## Filters

`query`, `wait`, `press` and `fill` take a filter; every field narrows it.

| Field | Matches |
|---|---|
| `text` | One of its texts or its tooltip, exactly. |
| `textLike` | Part of one, in any case. |
| `id` | Its `id`, exactly. |
| `type` | Its class or a subclass. |
| `role` | Its role. |
| `icon` | The named icon it shows. |
| `window` | Part of its window's title. |
| `under` | A handle it is inside. |
| `has` | A filter one of its descendants matches. |
| `enabled` | `true` or `false`. |
| `hidden` | `true` includes elements that are not on screen. |
| `index` | Only the match at this position, from 0. |

## Verbs

Every agent answers these:

| Verb | Arguments | Answer |
|---|---|---|
| `verbs` | | `{verbs: [{verb, about}]}`: the protocol, from the agent itself |
| `ping` | | `{pid}` |
| `windows` | | `{windows: [{h, title, …}]}` |
| `query` | filter | `{elements}`, empty when nothing matches |
| `wait` | filter, `absent?`, `timeoutMs` (10000) | `{elements, ms}`, or `timeout` |
| `read` | `h` | the element |
| `dump` | `under?`, `window?`, `hidden?` | `{text}`: the tree, one element a line |
| `activate` | `h` | `{how}`: what a click on it does, done |
| `set-text` | `h`, `text`, `activate?` | `{}`; `activate` then submits, as Return would |
| `toggle` | `h`, `active?` | `{active}` |
| `select` | `h`, `index` | `{}` |
| `focus` | `h` | `{focused}` |
| `settle` | `quietMs` (300), `capMs` (10000) | `{ms, settled}` |
| `screenshot` | `path`, `h?` | `{path, width, height}`: drawn by the app |
| `quit` | | `{}`, then the app quits as its own Quit does |

An agent may answer more where its platform has more to offer (`eval` into a web view and
`action` on Linux; `expand`, `collapse`, `scrollIntoView` and `scrollTo` on Windows; VoiceOver's
`escape` and `scroll`, and `increment` and `decrement`, on macOS and iOS), and lists them under
`verbs`.

The client adds two, done in two requests: `press` is `wait` for exactly one element, then
`activate`; `fill` is the same, then `set-text`.

## Refusals

| `code` | Means |
|---|---|
| `bad-request` | Not JSON, no verb, an unknown argument, or a wrong type. |
| `unknown-verb` | No such verb; `verbs` lists them. |
| `no-element` | No element has that handle, or a list has no item at that index. |
| `gone` | The element that handle named has gone: query again. |
| `timeout` | `wait` ran out; the reply says what was `near`, or what was still there. |
| `ambiguous` | `press` or `fill` matched more than one (the client's refusal). |
| `disabled` | A person could not act on it. |
| `not-showing` | It is not on screen. |
| `not-editable` | Text cannot be set there, or it is read-only. |
| `not-toggleable`, `not-selectable`, `not-activatable` | It has no such action. |

## Status per platform

| Platform | State |
|---|---|
| Linux | Speaks this protocol; it is the reference the conformance suite was written against. |
| Windows | Speaks this protocol and passes the conformance suite. A `role` is the UI Automation control type (`Button`, `Edit`, `ListItem`) and `text` the accessible name. An element's state and value come with `read`, not with `query` or `wait`, and nothing draws a named `icon`, so that filter matches nothing. Its own additions: `screen`, the physical screen rectangle a press is aimed in, and `window`, the HWND, on every element; a window's `h` is its HWND, which `window` takes as well as a title; the `signal` filter; `floorMs` on `settle`; `select` without `index` selects the element itself; the verbs `expand`, `collapse`, `scrollIntoView`, `scrollTo`, `focused`, `parent` and `collect`; and the refusals `not-expandable` and `failed`, the second for an error the framework raised. |
| macOS, iOS Simulator | Speaks this protocol and passes the conformance suite. It reads NSAccessibility or UIAccessibility in process, through the entry points VoiceOver's requests reach, and acts through accessibility actions. SwiftUI builds its elements only for an assistive technology, so the agent switches each hosting view's accessibility on, and on the simulator turns on the automation XCUITest uses, which loads UIKit's names for its own controls. A physical device has none, as its app cannot reach the Mac's socket: XCUITest drives it. A gesture or real input (a drag, a swipe action, a pull to refresh, a keystroke) stays with `macos-ax.swift` and idb. |

## Conformance

Start a client on the in-memory demo account (`MAILCAL_DEV_ACCOUNT=demo`) with its agent on, then:

```sh
scripts/dev/agent_conformance.py --socket "$MAILCAL_TEST_AGENT"
```

It drives only the protocol and the controls every client draws on the demo account, a phone's
included, so it runs unchanged on every platform. What one platform does differently it asks the
agent rather than assuming: macOS's view switcher is its sidebar, which leads back to mail through
the unified Inbox, and an iPhone folds its search field away once the view has changed, which the
search test then skips. An agent that passes it can be driven by `agent.py` and by the
suites built on it.
