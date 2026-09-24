# Writing style and drafted replies: cross-platform contract

**Scope.** What a writing style is, how one is learned from the person's own sent mail, how a reply
is drafted in it, what leaves the device on the way and what never does, and the gate every request
passes. "Settings → Writing style" and "Draft a reply" must mean the same thing on every platform.

**Principle.** Nothing is trained. A style is a short **description** of how a person writes plus a
handful of **passages** they wrote, and a draft is made from those, the message being answered and
what the person last wrote to the same recipient. The core owns the corpus, the prompts, the gate
and the library; a client renders them and words its own copy (localisation is client-side). The
draft goes into a composer the person edits; **the AI path never sends mail**.

## What a writing style is

Three layers, each carrying what the others cannot:

1. **The style guide** (`mailcal_ai::StyleGuide`), one section per language the person writes in:
   greetings and sign-offs with their exact wording and rough share, the name they sign with,
   register and how it shifts, typical length and number of paragraphs, paragraphing, punctuation,
   structural habits, how they decline or chase, characteristic phrases, what they avoid, and a
   heading of at most five words for the register, the structure and the declining and chasing. Plus the person's own **notes**,
   which every draft reads and which win over the description. The descriptive fields are written
   in the language of the person's interface, because they read them back; the habits are quoted
   in the language of the mail, because a draft copies them.
2. **Exemplars** (`mailcal_ai::Exemplars`): six to ten short passages per language, the person's own
   words, **picked by number from the device's copy**. The model names messages; it never returns
   their text, so an exemplar can never be a model's paraphrase.
3. **Recipient context**, at draft time: the person's own words from the last two messages they
   sent the same address. The strongest signal for register, and it needs nothing stored.

Styles are **standalone entities**, like signatures: a named library, and **one slot per account**
for the style it drafts in. Forgetting a style clears it from every account that used it; removing
an account drops its slot. A learned style takes the slot of the account it was learned from when
that slot was empty.

Both halves carry a `schema_version` and keep every field they do not model, so an older device
that edits a newer guide (its notes, its name) writes the newer fields back untouched.

## Learning

1. The person picks an account and a range: everything the device holds, everything up to a date
   ("until the end of 2024", to leave out what was written with an assistant's help), or a custom
   range.
2. **On the device**: the core reads that account's Sent folder in the range, newest first, and cuts
   each message to the words its author wrote. The quote is found by the attribution line in any
   catalog locale's wording (read from `messages/*.json` at build time, so a change of copy moves
   the stripper), a dash-framed forward marker, a header block, or three `>` lines in a row; the
   signature by the `-- ` delimiter or a trailing copy of one of the person's signatures. Calendar
   answers, automatic replies, repeats and anything under forty words of its own are dropped. Each
   message's language is named by a stopword count over the catalog's languages; a message in any
   other language is counted and left out. Each language is sampled under a token budget, longest
   first within each recipient, recipients in turn, most recently written-to first.
3. **The consent sheet**: `sent_corpus_report` runs step 2 and stops. The sheet says how many
   messages were found and how many say enough to learn from, per language, the date range, and
   exactly what will be sent where. Consent is given at the point of use: pressing **Learn** on
   that sheet is the act. There is no boot-time prompt.
4. **Learn**: one request per language (a sample larger than one request is described in parts and
   merged). The answer arrives through the `emit_json` tool, whose schema is derived from the type
   it is read into. A description is bounded on the way in, so a field cannot carry a paragraph of
   somebody's mail. Progress is on `Surface::WritingStyle`; the person can cancel before the next
   request.
5. **The reveal**: the description is shown back in plain words, one language at a time, the person
   adds notes, names the style and is offered a draft on a recent message. A greeting or sign-off
   is drawn by its part of its list, because a model's shares need not add up, and worded by its
   own share: half the messages or more is "mostly", a fifth or more "often", less "sometimes".
   The core computes both, so every client says the same.

Our own sent mail always carries a `text/plain` part, which is what the stripper reads; a message
with no plain part is converted from its sanitised HTML first.

## Drafting a reply

Replies only: reply and reply all. A new message from bullet points, and rewriting a draft in the
person's voice, are later uses of the same seams.

- **Input**: the message being answered as plain text (with whatever history it quotes), the
  style's section for the answer language (or its main one when it has none), that language's
  passages, the recipient context, the person's one-line intent when they give one, and the plain
  text of the signature the composer will add, so the body does not repeat it.
- **Language**: the chosen one, else the language of the message being answered, else the style's
  main language.
- **Output**: the body only, opening and closing the way the person does. Where the reply needs a
  fact that is in neither the thread nor the intent, the model writes a short bracketed gap
  (`[date]`); the draft lists them and the client says to check the parts in brackets. Where the
  person uses them, the draft may carry a small Markdown subset: `**bold**`, `*italic*`, lists
  whose lines start with `- `, `* `, `•` or `1.`, and a `#` line drawn as a bold line. The
  answer's ceiling guards against a runaway answer and is well above any reply's length; one that
  reaches it is logged.
- **Summary and checklist**: the same request answers with what the message asks of the person, in
  a sentence or two, and what they still have to do: each placeholder of the reply (listed by the
  core from the reply itself, so each item names the exact text), then files to attach and actions
  elsewhere (listed by the model, at most six). Both are in the interface language. The reply never
  says the person has done something they have not; the model writes it as something they will do,
  and lists it.
- **Where they show**: in a card between the composer's buttons and its text, never part of the
  mail, never saved or sent. A placeholder's item ticks itself once its text is gone from the reply
  (the client asks the editor's `composerPlaceholdersLeft`); the person ticks the others. At Send,
  with items still open, the client asks once whether to send anyway or keep editing; it never
  blocks the send.
- **Insertion**: into the **open** composer, above the signature and the quote, through the editor
  bundle's `setComposerDraftText`. It replaces only what is above those two regions, leaves both
  exactly where they are, builds the formatting as elements (never parsing markup), and puts the
  caret at the end of the draft. Nothing is sent. What the person sends is compared with the draft
  as the composer's plain text renders it, so the formatting itself is never counted as an edit.

## Keeps learning, from the person and never from the model

A style learned from a model's words would drift towards the model, so **a reply sent from a draft
is never learned from**, and never shown to a later draft as the person's own words.

- Every draft the core hands out carries an id. The client passes it to `setComposerDraftText`
  with the text, and the editor hands it back as `ai_draft` beside the document on submit.
- The reply path logs the sent message's `Message-ID` against that id, with the word-level
  difference between the draft and what the person actually sent above the signature and the
  quote: the runs they added, the runs they took out, the share of words that changed. A draft sent
  as it was carries no signal at all.
- The log (`writing_style_observations.toml`) stays on the device, is capped at two hundred
  entries, and loses a style's entries when the style is forgotten and an account's when the
  account is removed. Learning and the recipient context both pass over every logged message.
- Mail the person wrote without a draft stays eligible, as it always was.

## What leaves the device, and what never does

| What | Where it goes | When |
|---|---|---|
| The sample: the person's own words from their sent mail, fenced | The AI endpoint | A learning run the person started on the consent sheet |
| The message being answered, the style, the passages, the recipient context, the intent | The AI endpoint | A draft the person asked for |
| Each style's name, guide and notes (never the passages) | The Allodia account service, sealed | Every account-list sync of a device whose Allodia sign-in includes the writing-style scopes |
| Passages, quoted mail outside a request, attachment bytes, any address book | Nowhere | Never |

Every prompt follows the MCP server's shared bar ([`mcp.md`](mcp.md), "The shared bar", items 6 to
8): plain text only, fenced in `<untrusted-message-content>` with a one-line preamble and any closing
tag inside neutralised, no attachment bytes. There is no tool that sends, files or deletes. Nothing
here logs a prompt, a draft, a passage or an address: counts, stages and the gate's labels only
([`logging.md`](logging.md)).

## The gate

**Sovereignty scope.** This dispatch **passes the jurisdiction gate**; it is not a carve-out
([`../AGENTS.md`](../AGENTS.md), "Non-negotiables"). The gate is `mailcal-jurisdiction`: a
destination is classified, the class is compared with the mode, and a refusal names both.

- The mode is a preference, **`eu-native` unless the file says otherwise**. `eu-hosted` admits an EU
  data centre run from outside the EU; `all` admits anything, including an unclassified endpoint.
  No client offers to change it.
- **Allodia's relay is `eu-native` by construction**: the gateway behind it forwards only to
  providers it classifies as EU-native.
- **An own endpoint is what the person declared** (EU company in the EU, EU data centre of a
  non-EU company, outside the EU), and unknown until they declare anything. A model on the
  person's own computer or server in the EU is EU-native.
- The only thing that dispatches is a `GatedBackend`, whose `chat` asks the gate first, and every
  function that sends takes one; the app holds nothing else. A test pins that no public type in
  `mailcal-ai` implements the backend port, and another runs every mode against every destination
  and asserts nothing reaches a backend the gate refused.
- A refusal is reported **before anything is read**: the Writing style surface carries the gate's
  verdict, and learning and drafting ask the gate before touching the Sent folder.

## Where requests go

An own endpoint, when one is set up, wins; otherwise the relay, when the build has one and the
person's plan includes AI; otherwise nowhere, and the Writing style category is not shown.

**An own endpoint** is any server that speaks OpenAI's chat-completions API, set up under Settings →
Advanced: its base URL, a key, a model name and the declaration above. HTTPS is required, except to
this device (`localhost`, a loopback address), where a local model server listens. The key is in
the platform keystore under the reserved id `ai-endpoint`; the rest are preferences. A request is
not streamed, and waits at most five minutes for a learning request and two for a draft.

**The relay** (`POST /api/v1/ai/chat` on the Mail & Calendar service, bearer = the Allodia access
token with `mailcal:ai:use`) takes the same request **without** `model`, plus `purpose: "style" |
"draft"`, from which the gateway picks the model. It answers with one JSON object, not a stream:
the chat-completions `choices` and `usage`, plus `allodia: { credits_charged, balance_credits }`,
both as the gateway metered them. A refusal carries its code in `data.code`: `402`
`insufficient_credits`, `403` `not_entitled` (a plan without AI), `429` `rate_limited` (thirty
requests in ten minutes per person), `413` `too_large` (over 1 MiB), `502` `upstream_failed`, `503`
`unavailable`. The gateway answers the relay as a stream, which the relay collects into that one
object. It waits five minutes for a learning request and two for a draft, stores nothing of a
request and logs no content. `GET /api/v1/ai/balance` answers `{ balanceCredits,
startingGrantApplied }`; the first call on a plan with AI opens the person's credits with the
starting grant, once.

## Storage

| What | Where | Why there |
|---|---|---|
| The library: names, guides, passages, order | `writing_styles.toml` | Each style carries a few kilobytes of passages, and every preference write rewrites the whole preferences file. The guide and passages are stored as JSON strings, so the file never models the schema `mailcal-ai` owns. |
| Each account's style | `preferences.toml`, `[ai.writing_styles]` | A small per-account pointer. |
| The own endpoint's address, model and declaration | `preferences.toml`, `[ai.endpoint]` | Not secret. |
| The own endpoint's key | The platform keystore, id `ai-endpoint` | A secret. Taken out of the stored configs at boot before any mail parser sees it; a client asks `is_reserved_config` to tell a first run from a launch with mail accounts. |
| The jurisdiction mode | `preferences.toml`, `jurisdiction_mode` | It binds every external dispatch, not only these. |
| The last entitlement answer, and the balance the relay last reported | `preferences.toml`, `[ai]` | Derived, not secret; a launch without a network draws what it was last told ([`entitlement.md`](../allodia_license/entitlement.md)). Dropped at sign-out. |
| Each synced style's record id, the version last read, a fingerprint of what it held then, and the retry key of a create in flight | The Allodia sync bookkeeping (`SyncStateStore`), its `styles` beside the accounts' entries | Bookkeeping, not secret, and one blob with the accounts' so it is written whole. The fingerprint is what tells a change made here from one made elsewhere. Dropped at sign-out. |

A style's id is opaque CSPRNG output, never derived from its name.

## Per-platform matrix

| Capability | Shared core | macOS | iOS/iPadOS | Windows | Android | Linux |
|---|:---:|:---:|:---:|:---:|:---:|:---:|
| The gate, before anything is read or sent | ✅ | ✅ | ✅ | 🚧 | 🚧 | 🚧 |
| Learn a style: report, consent sheet, progress, cancel | ✅ | ✅ | ✅ | 🚧 | 🚧 | 🚧 |
| The reveal, notes, rename, forget | ✅ | ✅ | ✅ | 🚧 | 🚧 | 🚧 |
| Per-account style slot | ✅ | ✅ | ✅ | 🚧 | 🚧 | 🚧 |
| Draft a reply into the open composer, with gaps | ✅ | ✅ | ✅ | 🚧 | 🚧 | 🚧 |
| What the message asks, and a checklist asked about once at Send | ✅ | ✅ | ✅ | ⬜ | ⬜ | ⬜ |
| Own endpoint under Settings → Advanced | ✅ | ✅ | ✅ | 🚧 | 🚧 | 🚧 |
| Allodia relay: the entitlement read, requests, the balance | ✅ | 🚧 | 🚧 | 🚧 | 🚧 | 🚧 |
| Style guide synced between devices | ✅ | 🚧 | 🚧 | 🚧 | 🚧 | 🚧 |
| Fetch older sent mail back to a date | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ |

macOS and iOS were driven against the harness's Sent Items and the canned endpoint: the refusal
until the endpoint says where it runs, learning, the reveal, the library, the account slot, a draft
into a reply and the replace question, forget, and the category leaving with the endpoint; iOS on
an iPhone simulator. Windows, Android and Linux are built and not yet driven (Known gaps).

Legend: ✅ shipped · 🚧 in progress · ⬜ planned · — not applicable.

## Client seams

- **Methods** on `MailcalApp`: `writing_styles`, `writing_style_detail`, `rename_writing_style`,
  `update_writing_style_notes`, `delete_writing_style`, `set_account_writing_style`,
  `resolve_writing_style`, `sent_corpus_report`, `learn_writing_style`,
  `cancel_writing_style_learning`, `draft_reply`, `ai_available`, `own_ai_endpoint`,
  `set_own_ai_endpoint`, `clear_own_ai_endpoint`, `refresh_ai_balance`. **`sent_corpus_report`,
  `learn_writing_style`, `draft_reply` and `refresh_ai_balance` block**: a client calls them off the
  main thread.
- **`Surface::WritingStyle`** is signalled when the library, an assignment, the backend or a
  learning run's progress changes. Its snapshot's `route` says whether AI is available at all, and
  a client shows the Writing style category only when it is `Some`.
- **`setComposerDraftText(text, draftId)`** in the editor bundle inserts a draft, as described
  under "Drafting a reply", and keeps `draftId` (`DraftReply.draft_id`) for the submit.
- **`composerPlaceholdersLeft(placeholders)`** answers which of a draft's placeholders are still in
  the reply above the signature and the quote, so a checklist can tick its fill-in items.
- **`composerLeadHasText()`** answers whether the person has already written above the
  signature and the quote; a client asks before a draft replaces it, and asks to replace.
- **A failure** is a `WritingStyleFailure` variant, never a server's sentence; a client words it.
- **Driven locally** against the harness's Sent Items and a canned endpoint, with no credits
  spent ([`debugging.md`](debugging.md), "Start the local mail server").

## Known gaps

- **Windows, Android and Linux are built and not driven.** Windows and Linux have compiled only in
  CI; Android's JVM suite passes and its debug build succeeds. iPad was not driven either.
- **"Try it on a recent message" is not offered** on any client; the reveal ends at Save.
- **No driven run has sent a reply from a draft**, so the `ai_draft` round trip is proven by the
  editor's and the core's tests, not on a screen.
- **On Linux, a composer window left open** does not see a style learned meanwhile until From
  changes or the reply is opened again.
- **Drafts have not been judged against the relay's providers.** One of them is known to fold a
  model's reasoning into its answer text; the first drafts through the relay are to be checked for
  it, and an evaluation harness is not built.
- **The relay service is not deployed.** The device side is built against the request and answer
  fixed above: a signed-in account whose entitlement grants `ai` goes through the relay, the
  entitlement is read in the background and kept in the preferences, and every answer's balance is
  recorded. Buying credits is not built; running out is an honest "no credits left".
- **No client draws a style conflict yet.** A style changed here and on another device arrives in
  `AllodiaSyncReport.style_conflicts` by name, and nothing of either side is applied until
  `resolve_writing_style_conflict` keeps one; until a client offers that choice, such a style
  stays as it is on each device. The service's cap of twenty styles is only logged.
- **A synced style's passages are picked once**, when it arrives, from the sent mail every account
  on the device holds at that moment. A device that holds none yet, a new one most of all, has none
  until the style is learned there, and drafts from the guide and the recipient context alone.
- **Learning reads what the device holds**: the Sent folder within the account's sync depth, three
  months by default. The consent sheet reports the device's horizon; fetching older sent mail is not
  built.
- **The thread is the message being answered**, with whatever history it quotes; older messages of
  the conversation are not read separately.
- **The corrections are recorded but nothing learns from them yet.** A refinement that updates a
  style from them, a "not like me" button, and a line saying how much drafts still need editing are
  not built.
- **A draft saved to the server and resumed later loses its id**, so a reply sent from it is treated
  as the person's own; so is one sent after a restart, because the ids a session issued are kept in
  memory.
- **The credit cost of a learning run** is not estimated on the consent sheet: the token count is
  known on the device, the price only at the relay.

## Enforcement

This contract is binding via [`../AGENTS.md`](../AGENTS.md). When you change what is learned, what is
sent, where it goes or how a draft enters the composer:

1. Update this document, **the rule and the matrix**, in the same change.
2. Apply the change to every platform that has the surface, or record the shortfall under Known
   gaps.
3. Update [`capabilities.md`](capabilities.md) when a capability's reach shifts, and add a changelog
   fragment when a person could notice.
4. Anything that widens what leaves the device updates [`privacy-policy.md`](privacy-policy.md) and
   both its locales, with the version and date line bumped.

Automated:

- `crates/mailcal-jurisdiction/src/tests.rs`: the mode table, the classification, the stored labels.
- `crates/mailcal-ai/src/gated_tests.rs`: nothing reaches a backend the gate refused, and no public
  type implements the backend port.
- `crates/mailcal-ai/src/corpus/strip_tests.rs`, `corpus_tests.rs`, `sample_tests.rs`,
  `language_tests.rs`: the stripper per catalog locale, the filters, the budget, the detector held to
  the catalog.
- `crates/mailcal-ai/src/learn_tests.rs`, `draft_tests.rs`: exemplars are the device's own text, the
  fence cannot be closed from inside, gaps are listed.
- `crates/mailcal-app/src/writing_style_tests.rs`, `writing_style_ai_tests.rs`: no dangling slot,
  only the author's words leave the device, and a reply sent from a draft is logged and never
  learned from.
- `crates/mailcal-ai/src/observe.rs`: a draft sent as it was yields no signal, an edited one only
  its edited runs.
- `crates/mailcal-bindings/src/ai_transport_tests.rs`, `tests_ai_endpoint.rs`: the request over a
  real socket, the key in the keystore and back at the next launch.
- `clients/composer/tests/seeds.test.ts`: a draft replaces only what is above the signature and the
  quote.
