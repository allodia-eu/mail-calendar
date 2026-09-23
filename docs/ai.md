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
   register and how it shifts, typical length, paragraphing, punctuation, structural habits, how
   they decline or chase, characteristic phrases, what they avoid. Plus the person's own **notes**,
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
5. **The reveal**: the description is shown back in plain words, the person adds notes, names the
   style and is offered a draft on a recent message.

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
  (`[date]`); the draft lists them and the client says to check the parts in brackets.
- **Insertion**: into the **open** composer, above the signature and the quote, through the editor
  bundle's `setComposerDraftText`. It replaces only what is above those two regions, leaves both
  exactly where they are, and puts the caret at the end of the draft. Nothing is sent.

## What leaves the device, and what never does

| What | Where it goes | When |
|---|---|---|
| The sample: the person's own words from their sent mail, fenced | The AI endpoint | A learning run the person started on the consent sheet |
| The message being answered, the style, the passages, the recipient context, the intent | The AI endpoint | A draft the person asked for |
| The style guide and notes (never the passages) | The Allodia account service, sealed | When the person's plan syncs styles (see Known gaps) |
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
the chat-completions `choices` and `usage`, plus `allodia: { credits_charged, balance_credits }`.
`402` is out of credits, `403` with code `not_entitled` is a plan without AI. The relay stores
nothing and logs no content.

## Storage

| What | Where | Why there |
|---|---|---|
| The library: names, guides, passages, order | `writing_styles.toml` | Each style carries a few kilobytes of passages, and every preference write rewrites the whole preferences file. The guide and passages are stored as JSON strings, so the file never models the schema `mailcal-ai` owns. |
| Each account's style | `preferences.toml`, `[ai.writing_styles]` | A small per-account pointer. |
| The own endpoint's address, model and declaration | `preferences.toml`, `[ai.endpoint]` | Not secret. |
| The own endpoint's key | The platform keystore, id `ai-endpoint` | A secret. Taken out of the stored configs at boot before any mail parser sees it; a client asks `is_reserved_config` to tell a first run from a launch with mail accounts. |
| The jurisdiction mode | `preferences.toml`, `jurisdiction_mode` | It binds every external dispatch, not only these. |

A style's id is opaque CSPRNG output, never derived from its name.

## Per-platform matrix

| Capability | Shared core | macOS | iOS/iPadOS | Windows | Android | Linux |
|---|:---:|:---:|:---:|:---:|:---:|:---:|
| The gate, before anything is read or sent | ✅ | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ |
| Learn a style: report, consent sheet, progress, cancel | ✅ | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ |
| The reveal, notes, rename, forget | ✅ | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ |
| Per-account style slot | ✅ | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ |
| Draft a reply into the open composer, with gaps | ✅ | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ |
| Own endpoint under Settings → Advanced | ✅ | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ |
| Allodia relay, credits and balance | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ |
| Style guide synced between devices | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ |
| Fetch older sent mail back to a date | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ |

Legend: ✅ shipped · 🚧 in progress · ⬜ planned · — not applicable.

## Client seams

- **Methods** on `MailcalApp`: `writing_styles`, `writing_style_detail`, `rename_writing_style`,
  `update_writing_style_notes`, `delete_writing_style`, `set_account_writing_style`,
  `resolve_writing_style`, `sent_corpus_report`, `learn_writing_style`,
  `cancel_writing_style_learning`, `draft_reply`, `ai_available`, `own_ai_endpoint`,
  `set_own_ai_endpoint`, `clear_own_ai_endpoint`. **`sent_corpus_report`, `learn_writing_style` and
  `draft_reply` block**: a client calls them off the main thread.
- **`Surface::WritingStyle`** is signalled when the library, an assignment, the backend or a
  learning run's progress changes. Its snapshot's `route` says whether AI is available at all, and
  a client shows the Writing style category only when it is `Some`.
- **`setComposerDraftText(text)`** in the editor bundle inserts a draft, as described under
  "Drafting a reply".
- **A failure** is a `WritingStyleFailure` variant, never a server's sentence; a client words it.

## Known gaps

- **No client draws any of it yet.** Every client cell in the matrix is ⬜.
- **The relay, credits, the balance and style sync** are not built. The relay's request and answer
  are fixed above so both sides can be built against them.
- **Learning reads what the device holds**: the Sent folder within the account's sync depth, three
  months by default. The consent sheet reports the device's horizon; fetching older sent mail is not
  built.
- **The thread is the message being answered**, with whatever history it quotes; older messages of
  the conversation are not read separately.
- **Corrections are not captured yet.** What the person changes in a draft before sending, which is
  what a later refinement of the style learns from, is not recorded.
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
  and only the author's words leave the device.
- `crates/mailcal-bindings/src/ai_transport_tests.rs`, `tests_ai_endpoint.rs`: the request over a
  real socket, the key in the keystore and back at the next launch.
- `clients/composer/tests/seeds.test.ts`: a draft replaces only what is above the signature and the
  quote.
