# Writing style and drafted replies: cross-platform contract

**Scope.** What a writing style is, how one is learned from the person's own sent mail, how a reply
is drafted in it, what leaves the device, and how each client draws the three surfaces that carry
it: Settings → Writing style, the own endpoint under Settings → Advanced, and **Draft a reply** in
the composer.

**Principle.** Nothing is trained. A style is a short **description** of how a person writes and a
handful of **passages** they wrote, learned on the device from their own sent mail; a draft is made
from those, the message being answered and what the person last wrote to the same address. The
core decides every surface (whether it exists, what it holds, which control is held and why, what a
failure offers); a client draws it and words it. A draft goes into a composer the person edits,
and **the AI path never sends mail**.

## What a style is

- **The description** (`mailcal_ai::StyleGuide`), per language: greetings and sign-offs with their
  wording and share, the name signed with, register, length and paragraphing, punctuation, how a
  reply is built, how the person declines or chases, phrases and what they avoid. Descriptive
  fields are in the interface's language, because the person reads them; habits are quoted in the
  mail's, because a draft copies them. The person's **notes** sit beside it, and win over it.
- **The passages** (`mailcal_ai::Exemplars`): six to ten per language, picked **by number** from
  the device's copy. The model never returns their text, so a passage is never its paraphrase.
- **The recipient context**, at draft time: the person's words from the last two messages they sent
  the same address. Nothing of it is stored.

Styles are standalone, like signatures ([`signatures.md`](signatures.md)): a named library, and one
slot per account, a style or **None**. Forgetting a style clears every slot holding it; removing an
account drops its slot; a learned style takes its account's slot when that is empty; a slot given a
style that names nothing is cleared. A style's id is opaque CSPRNG output.

## The rules

### Where requests go

1. **Every request passes the jurisdiction gate before anything is read.** Only
   `mailcal_ai::GatedBackend` dispatches a model request, and it asks `mailcal-jurisdiction` first;
   learning and drafting ask before they touch the Sent folder or the message answered. The mode is
   `eu-native` unless the preferences say otherwise; `eu-hosted` admits an EU data centre run from
   outside the EU, `all` admits anything. No client offers to change it.
2. **An own endpoint wins; otherwise the relay; otherwise nowhere.** An own endpoint is any server
   speaking the OpenAI chat-completions API, classified by what the person declared: in the EU by a
   European company or on their own computer (`eu-native`), in the EU by a company based outside it
   (`eu-hosted`), or outside the EU; undeclared, it is unclassified, which only `all` admits. The
   relay (`POST /api/v1/ai/chat` on the Mail & Calendar service, bearer the Allodia access token
   with `mailcal:ai:use`) is `eu-native` by construction: its gateway forwards only to providers it
   classifies as EU-native, picks the model by `purpose`, and stores and logs nothing of a request.
   A request waits five minutes to learn and two to draft.
   The relay is a request's route only in a build that carries the Allodia sign-in, and only
   while an account is signed in. It sends only to the account service fixed at build time
   (`allodia_license::host()`), so nothing else can be sent under the relay's class. It is ready unless the service
   refused the sign-in or the grant is recorded without `mailcal:ai:use`; either is
   `RelaySignIn`, said before anything is pressed, and the scope raises no prompt anywhere else.
   The body is the chat-completions request with `purpose` (`style` or `draft`) and without a
   model. The relay's refusal codes reach the person as failures (rule 27): `401` the sign-in,
   `402 insufficient_credits` no credits, `403 not_entitled` the entitlement, which the core then
   reads again at once, `429 rate_limited` too many requests, `413 too_large` the sample, and
   anything else a server error.
3. **What leaves is the request the person started.** Learning sends their own words from the range
   they chose, without quotes or signatures. Drafting sends the message answered, the style's
   section and passages for its language, the recipient context, the instruction and the plain text
   of the signature the composer will add. Never an attachment, an address book or a passage
   outside a request. Every prompt meets [`mcp.md`](mcp.md)'s shared bar, items 6 to 8; no tool the
   model is offered sends, files or deletes; nothing logs a prompt, a draft, a passage or an address
   ([`logging.md`](logging.md)).

### When the feature is offered

4. **The three surfaces appear and disappear together, on one decision.** The Writing style
   category, Draft a reply and the own endpoint form are drawn exactly when `ai_offer()` says the
   feature is offered; each surface's record reads it, so no client asks apart. Not offered, no
   request can be built (the route answers `NotEntitled`), and an endpoint set up earlier is kept.
   The licence side tells the core what the entitlement says (`set_ai_entitlement`); a build
   without it never does, so it is production without `ai`.
5. **Early access: a development build offers it to everyone, a production build only while the
   Allodia entitlement carries `ai`.** A development build is a debug build, or the Android
   development build, whose core carries `dev-harness`. The entitlement is read as
   [`entitlement.md`](../allodia_license/entitlement.md) says, and an answer without `ai` takes
   effect at once: the stored answer inside its grace, never a read anyone waits for, published to
   the offer at launch, after each read, on each refresh trigger and at sign-out, so a grace that
   runs out takes the offer with it. Relay and own endpoint are offered together under it. Early access ends before
   the relay is sold; from then the own endpoint is in every build, as the relay's free counterpart
   ([`pledge.md`](pledge.md)).

### The own endpoint, under Settings → Advanced

6. **Four fields, tidied and judged by the core**: the address (HTTPS, except to `localhost` or a
   loopback address), the key (may be empty), the model, and where it runs. The core trims every
   field, the key included. What a save would refuse is said beside its field before Save is
   pressed, by `ai_endpoint_gap`, the function the save acts on: an unusable address, one needing
   HTTPS, no model, nothing declared, a declaration the gate refuses in the current mode.
7. **Save shows it is working and says what failed.** The form is a record of its own,
   `ai_endpoint_form()`, which the Advanced page embeds, read again on every `Surface::AiEndpoint`
   and `Surface::Settings` signal and `None` while the feature is not offered. Save is a change on
   it ([`waiting.md`](waiting.md)), an `AiEndpointIntent`, running from the press until the key is
   in the platform keystore (id `ai-endpoint`) and the rest in the preferences. The fields are held
   with it, drawn from the edit being saved, then from an edit a save refused, otherwise from what
   is stored. A stored key is never drawn back. A key typed replaces it. **A key goes only to the
   server it was given for**: a save whose address is on another host or port than the stored
   endpoint's, compared as the URL parser normalises them with a scheme's default port counting as
   none given, takes the kept key out unless a key is typed in the same save, and a key kept with
   no endpoint stored goes to no address; a change of path or of scheme alone keeps it, writing
   nothing to the keystore. What a save would do to it is said before Save is pressed, by the same
   `ai_endpoint_gap`, for the address as typed (`kept_key`: carried or dropped), and the key field
   says a key is kept only while it would be carried. A key the keystore refused, to store or to
   take out, is said on the form and changes nothing. **Remove** is the same kind of change, drawn
   only while an endpoint or a key is stored, and takes both out. Saving dials nothing; the first
   request says whether the endpoint answers.

### Settings → Writing style

8. **The page is `writing_style_page()`**, read again on every `Surface::WritingStyle` and
   `Surface::Settings` signal, changed by `WritingStyleIntent`s that return at once. Every intent
   of the feature travels as `Intent::Ai`, one family per surface. It holds the
   library's rows (name, the address learned from, how many messages, when, which languages), a
   slot picker per account used for mail once the library holds a style, and **Learn**. A library
   change (a rename, the notes, a slot, Forget, leaving the reveal) runs one at a time until
   written, holds the page, runs its own control and says what it came to as the page's
   `outcome` with a serial, as Signatures does; one not written is the `SettingsNotSaved` notice
   ([`notices.md`](notices.md)).
9. **Learn says on the page why it cannot be pressed**: no account is used for mail, there is
   nowhere to send, the gate refuses the endpoint as declared, or the Allodia sign-in needs
   renewing, each with its one remedy (rule 27). While a run goes, the page draws it in Learn's
   place, with its progress, the address whose sent mail it reads (`learn_progress_account`) and
   Stop; until the run knows how many requests it makes it says `learn_progress_starting`.
10. **A row opens the reveal once, and the detail ever after** (rules 15 and 16).

### Learning a style

11. **The flow is one record, `writing_style_learning()`, whose stage the core moves:**
    - `ChooseAccount`, only with more than one account used for mail, each named by its address.
    - `ChooseRange`, from the core's presets (`learn_ranges`): everything the device holds;
      everything up to the end of last year, to leave out mail written with an assistant's help;
      or a range of the person's own within days the core bounds, offered last and carrying the
      days it starts from (the earliest day the device holds, or a year back when it holds
      everything, to today). Only what the device holds is offered: the record says how far back that is (the account's sync depth), the end of last
      year is offered only when the device holds mail from before this year, and a range of the
      person's own starts no earlier. A day is a whole day in UTC, as a sync window's first day
      is. A client converts no dates.
    - `Review`: the core reads the range and stops before anything leaves: `Reading`, then `Ready`
      (messages found, how many say enough to learn from per language, how many were left out in
      other languages, the dates read) or `NothingUsable`. It says what is sent where, naming the
      relay or the endpoint's host, and names no price, which only the relay knows. At
      `NothingUsable` Learn is held and nothing will be sent, so the record names neither a
      destination nor a reason it could not send (`sends_to` and `cannot_send` are both empty).
      It says how far back the device holds the account's mail, and, for an account whose server
      keeps no keyword, that only this device knows its drafted replies (rule 25). **Pressing
      Learn there is the consent**; nothing asks anywhere else. Learn carries the default name the
      style is stored under and the language the app is read in, both the client's, since the
      core has no words or locale of its own.
    - `Learning { done, total }`, one request per language and part, with **Stop**, which ends the
      run before the next request and keeps nothing.
    - `Failed { failure, remedy }`, with its one remedy (rule 27). A client sends `Remedy` for
      every remedy, here and on the page's run, and never a `Close` with it. `Remedy` moves the
      flow for `ChooseAccount` and `ChooseRange` as Back does, and `TryAgain` runs again what
      failed. For `SignInToAllodia` it also goes back as Back does, and the flow stays open on
      the review while the client starts the sign-in, so Learn waits there for the person's
      return. For `OpenAiEndpoint` it ends the flow as Close does, and the client then opens
      Settings → Advanced. A `Close` sent before or after it comes to the same.
    - `Learned { style }`: the style is in the library under the default name, and the reveal
      follows in the same host.
12. **Back and closing do what the stage says.** Back, Next, Learn, Stop and Close are each a
    `ControlState` on the record; a client sends the intent.

    | Stage | Back | Closing the host |
    |---|---|---|
    | `ChooseAccount` | none | ends the flow, keeping nothing |
    | `ChooseRange` | to `ChooseAccount`, where it was shown | ends the flow |
    | `Review` | to `ChooseRange`, dropping a read under way | ends the flow, dropping a read under way |
    | `Learning` | none; Stop returns to `Review` | the run goes on, drawn on the page with its Stop ([`waiting.md`](waiting.md) rule 3) |
    | `Failed` | to the stage its remedy names, otherwise `Review`, or `ChooseRange` when nothing was read | ends the flow |
    | `Learned` | none | the reveal is left as its own Close leaves it (rule 15); the style keeps its default name |

    The record's `trail` names the stages before the current one, first to last, each where Back
    from the one after it goes, so a host that draws the flow as a navigation stack keys its path
    on it: `ChooseAccount` only when it was shown, then `ChooseRange`, then `Review`. `Learning`
    and `Learned`, which have no Back, stand on the review they were started from; a `Failed`
    stands on where its Back goes.

13. **One run at a time**, as queued work. A run and a draft never wait for each other: each
    kind of work keeps to its own lane (rule 20), and the review's read sends nothing and runs
    beside both. Opening the flow while one runs opens it on `Learning`; an answer for a range or an
    account the person has since left is dropped by the core. A run whose host was closed and that
    then learns leaves the flow, its style's row opening the reveal; one that fails is drawn on the
    page in Learn's place, and Learn reopens the flow on the failure.
14. **Only the person's words are read**: each sent message cut to what its author wrote, without
    quote or signature, and calendar answers, automatic replies, repeats, anything under forty words
    of its own and every drafted reply (rule 25) left out.

### The reveal and the detail

15. **The reveal shows a new style back once, in six pages the core composes**
    (`writing_style_reveal(style)`): which field is on which page, an empty one left out, the
    letter's line lengths, the first sentence and the languages are data; a client lays them out
    and animates them, drawing every value final under reduced motion. A page no language has
    anything for is left out, and a per-language page says so for a language with nothing for it.
    A description without a heading of its own is headed by its first sentence, which ends at a
    full stop, question mark, exclamation mark or ellipsis followed by the end or by a space and
    anything but a lower-case letter; the heading drops its full stop, a doubled one included, and
    keeps an ellipsis, written out or not, a question or exclamation mark and the point closing an
    abbreviation such as "e.g.". The pages: **what was read**
    (how many messages, from which address, since when, in which languages); **a typical reply** (a
    miniature letter in the person's usual length and shape, opening as they often do, which is one
    accessible element with its grey lines hidden, spoken as `a11y_letter_opening`,
    `a11y_letter_words`, `a11y_letter_paragraphs`, `a11y_letter_closing` and `a11y_letter_name` in
    that order, joined by a comma, each left out when its value is empty or zero: a greeting or a
    sign-off is its runs' text read as written, a placeholder as its word, and the paragraphs are
    the record's `paragraph_count`, never the lines drawn);
    **greetings and sign-offs**, each worded by its share ("mostly" for half or more, "often" for a
    fifth or more, "sometimes" below), with the name signed; **tone and approach** (register, how a
    reply is built, declining and chasing, punctuation); **phrases** used and avoided; and **name and
    notes**, with Save. The four in between are per language, with a language control when there is
    more than one. Moving between the pages is a control of the record (`paging`), held while a
    change to the library runs, as Save and closing are. The reveal counts as shown once it is
    left, by Save or by closing. **Closing is one control**: over a reveal the host draws one
    Close, from the reveal's `close`, and sends `CloseReveal`, whether the reveal follows learning
    or was opened from a row. After learning, the flow's `close` is that control in the same state
    and the flow's `Close` makes the same change, so either intent leaves the reveal once and the
    flow ends when the reveal is written as shown.
16. **An existing style opens its detail** (`writing_style_detail(style)`): the first page's facts,
    the reveal's sections as one page without paging, those the record lists (`sections`, left out
    by the rule the reveal leaves a page out by), the name, the notes and **Forget**. A blank
    name is refused (`writing_style_name_refusal`), its Save held meanwhile; the notes are sent on
    Return and on leaving the field. Forget is asked first, names the accounts that draft in the
    style, says no mail is touched, and runs until the library is written.

### Draft a reply

17. **Offered on a reply and a reply all, for the account it is sent from.** The composer reads
    `composer_reply_draft(composition, from)` with its From, again on `Surface::ReplyDraft` and
    `Surface::WritingStyle` and when From changes, so a From change or a style learned meanwhile
    reaches an open composer. It is `None` when the feature is not offered or the composition is
    not a reply; otherwise its `state` is `Unavailable { failure, remedy }`, `Ready`, `Drafting`,
    `Drafted { summary, checklist }` or `Failed { failure, remedy }`. A reply resumed from Drafts or
    the Outbox is a reply too, its message found by the `Message-ID` it answers. The control sits in
    the composer's action row, beside Signature.
18. **A Draft a reply that cannot draft says why where the person looks**, never in a tooltip
    alone. For a reason they can act on (no style yet, this account drafts in none, nowhere to send,
    the endpoint refused by the gate, the Allodia sign-in to renew) the control can be pressed and
    opens the instruction input with the reason (`held_by`) and its remedy in place of the field.
    For one they cannot (the message answered is no longer on this device, found when a draft is
    asked for) the control is held and the reason is a line among the composer's notes.
19. **The instruction is one optional line, in the platform's own control** (the flow host table),
    with the core's suggestions under it: yes, no, more information needed, a holding reply. A
    suggestion fills the line; Draft, or Return in the line, starts the draft. The core trims the
    line, and the input opens holding the one the last draft was asked with (`instruction`).
20. **Drafting returns at once and can be stopped.** `ReplyDraftIntent::Draft` starts it and `Stop`
    abandons it, the request in flight included, and an answer that comes after Stop is dropped;
    the composer stays usable. One indicator shows the work ([`waiting.md`](waiting.md) rules 1 and 3):
    Draft is pressed in the input the action row's control opened, and the input closes onto that
    control, which runs (`control`) until the draft is in or stopped and is the one spinner drawn.
    `Drafting` draws among the notes as a line with Stop and no spinner of its own, and Stop is
    never running, since it ends the work at once. A draft never waits
    for a style being learned or for another composer's draft: a composition drafts one reply at a
    time, and closing it stops its draft.
21. **A draft goes in above the signature and the quote, as one undo step.** The client runs the
    record's `editor_script` (`setComposerDraftText`) once per `serial` in each editor; the bundle
    replaces only what is above those two regions, builds its formatting as elements (`**bold**`,
    `*italic*`, list lines, a `#` line drawn bold) without parsing markup, and leaves the caret at
    its end. The insertion is one step of the editor's history ([`undo.md`](undo.md) rule 4), so
    one undo takes it out whole and one redo brings it back. It is neither a seeded body (undo rule
    7) nor a host change kept outside the history (undo rule 8): the editor reports after each
    change whether the draft is in, and the card follows the step, gone while it is undone and back
    when it is redone, so the control and the message never disagree. Until the editor reports the
    draft in, the record is `Drafting`; from then the script only has an editor opened later on the
    same composition (a composer moved into a window of its own) follow the draft in its body.
22. **Replacing what is there is asked first.** Once each seed that loads the body has run, and
    after each change, the editor reports what the text above the signature and the quote holds:
    nothing, the draft untouched, or anything else, which is the person's. The quoted original and
    the signature a reply opens with are not the person's writing. A client hands every message on
    the editor's channel to `composer_reply_draft_report`, which takes that report and nothing
    else. While the text holds the person's words, an edited draft included,
    `ask_before_replacing` holds and the client asks before Draft. Before the editor has reported,
    the core goes by what the composer opened with: a fresh reply is not asked about, while one
    opened holding a body of its own (a stored draft resumed, a prefill's text, a message taken
    back from the Outbox) is asked about until the editor says what that body holds.
    `Draft { replace }` carries the answer, and the core refuses a press that would replace
    unasked. A draft arriving after the
    person wrote there meanwhile waits (`replace_arrived`) and is asked about again
    (`AnswerArrived`). Cancel keeps their text and drops the draft.
23. **The card says what the message asks and what is left to do.** It sits among the composer's
    notes ([`composer-layout.md`](composer-layout.md) rule 1), is never part of the mail, saved or
    sent, and holds a summary of a sentence or two and a checklist. The checklist names each gap the
    draft left in brackets, listed by the core from the reply itself, then files to attach and
    things to do elsewhere, at most six. Whether an item is done is one function,
    `mailcal_ai::tasks_done`: a gap while its text is gone from the reply, open again when an undo
    brings it back; an attach item once as many files were added after the draft went in as there
    are attach items, since which file answers which item cannot be told, or once ticked; the rest
    once the person ticks them (`ReplyDraftIntent::Tick`). A gap is never ticked by hand: its item
    has no `tick`, and a client draws its `done` as a state beside the text (an open or a done
    mark that cannot be pressed), never as a disabled checkbox, which would read as unavailable
    rather than as following the reply. After each change above the signature and the quote the
    editor reports which of the draft's gaps are left, by their place in its list, and the core
    re-derives the ticks from that, never on a timer. A file added to or removed from the
    composition signals `Surface::ReplyDraft` when it changes an item or the question Send asks
    (rule 24), so a client reads the record again on that signal and on nothing of its own.
24. **Send asks once while the checklist has open items.** The record's `send_question` and
    `submit_composition` read the same function, so the two cannot disagree: while the question is
    due a client asks before it submits, and `submit_composition` refuses without sending.
    **Send anyway** and **Keep editing** both send `SendQuestionAnswered`, applied before `dispatch`
    returns; Send anyway then submits. Neither is asked again in this composition.
25. **A drafted reply is never learned from, nor shown to a later draft as the person's words.** A
    composition a draft went into stays drafted, even once undone or rewritten, since what is sent
    may still carry its words; the core holds that on the composition for as long as it is open,
    never in the editor's document. The send logs the `Message-ID` on this device and asks for the
    keyword `writing-assistant` on the Sent copy (the engine's `sent_copy_keywords`), fixed and
    untranslated so every device recognises it. Gmail keeps it as a label and Outlook as a
    category, which the person sees: created as "Writing assistant" and recognised under its name
    in every catalog locale, one table in `mailcal-account` holding all seven
    (`drafted_keyword.rs`). The engine sets no keyword on a draft it stores, so a stored draft
    carries none: a drafted reply saved, closed and resumed opens as not drafted, and is marked
    only if a draft goes into it again. A message is drafted when its listing carries the keyword
    **or** the log names it. A keyword never delays or fails a send or a save. On a server that
    keeps no keyword the log is the only sign, so another device can take that reply for the
    person's own, and the Review says so for such an account.
26. **A draft is the body only**, in the message's language when the style has it, otherwise the
    style's main one; under a signature it ends at its last sentence. The thread it answers is the
    message with the history it quotes, which is what the composer shows. Working notes or an
    unfinished answer are never a draft.

### Failures, and what a client decides

27. **A failure and its remedy are decided once, in the core** (`AiFailure`, `AiRemedy`); a client
    words them and never shows a server's sentence. Stop is not a failure. Every failure, remedy,
    refusal and endpoint gap has its message in the shared catalog (`ai_failure_*`, `ai_remedy_*`,
    `ai_endpoint_gap_*`, a refusal of an undeclared endpoint `ai_failure_refused_undeclared`), and
    a bindings test (`records_ai_words_tests.rs`) names the one for each variant, so no client
    adds words of its own. A remedy comes with its control (`AiRemedyControl`), wherever it is
    drawn: held while the surface it sits on holds its controls (a change to the library, on the
    page; Draft, for the composer's Try again) and never running, since what it starts is drawn
    where it leads. What it leaves is decided here: the learning flow as rule 11 says; in the
    composer only the instruction input, the client's own, and never the composer, whose record
    follows on `Surface::WritingStyle` (rule 17), no intent going to it: `LearnStyle` opens the
    learning flow (`StyleLearningIntent::Open`), `ChooseStyle` Settings → Writing style,
    `OpenAiEndpoint` Settings → Advanced and `SignInToAllodia` the Allodia sign-in. A refusal by
    the gate names only what the gate can refuse: a mode that refuses something and a class it
    refuses, never `all` and never an endpoint declared EU-native.

    | Failure | Remedy |
    |---|---|
    | The relay refused the Allodia sign-in, or it lacks `mailcal:ai:use` | `SignInToAllodia` |
    | The own endpoint refused the key; the gate refuses the endpoint; nowhere to send | `OpenAiEndpoint` |
    | Unreachable, an error status, too many requests, an answer that cannot be read | `TryAgain` |
    | No credits left; the entitlement lost `ai` (the feature then stops being offered) | none |
    | The account has no Sent folder | `ChooseAccount` when there is another, otherwise none |
    | Nothing long enough to learn from; a sample too large to send | `ChooseRange` |
    | No style learned yet; this account drafts in none | `LearnStyle`; `ChooseStyle` |
    | The message answered is no longer on this device | none |

28. **A client decides the words, the layout, the animation and the native control, and nothing
    else**: never whether a surface exists, which stage is next, whether a control is held or why,
    whether an item is ticked, whether to ask before replacing or at Send, or which remedy a
    failure offers. Every control the core gives a state goes through the client's control
    components ([`waiting.md`](waiting.md)).

## The flow host

Each client has **one host for a multi-step flow**, named for what it is rather than for this
feature, since account setup is to use it too. It draws the core's record (the title, the stage,
Back, the primary action, Close, each from its `ControlState`, and a step count only when the record
gives one) and holds no step, index or rule of its own; in the reveal it keeps only which page is on
screen. At the platform's largest text size a button's label wraps between words, or the buttons
stack; it never breaks inside a word.

| Platform | Host | The stages | The reveal's pages | The instruction input |
|---|---|---|---|---|
| macOS | a sheet over the Settings window | Cancel at the leading edge of the button row; Back and the primary at its trailing edge, the primary the default button | the same sheet, page dots between Cancel and Back | a popover from Draft a reply |
| iOS/iPadOS | a sheet holding a `NavigationStack`, full height on iPhone, a page sheet on iPad | each stage pushed; the back button and the edge swipe are Back, Cancel at the leading toolbar placement, the primary at the confirmation placement | a paged `TabView` with its page indicator | a popover from the control on iPad; a sheet at half height on iPhone |
| Windows | the Settings dialog's detail panel, since WinUI forbids a nested `ContentDialog` | a footer: Cancel, then Back, then the primary in the accent style | a `PipsPager` in the footer | a `Flyout` from the control |
| Android | a full-screen dialog over Settings, as the signature editor is | a top app bar with Close; system back and its predictive gesture are Back where the stage has one; the primary in a bar at the foot | a `HorizontalPager` with a page indicator | a `ModalBottomSheet`, never an `AlertDialog` |
| Linux | the Settings window's detail, never a second toplevel ([`signatures.md`](signatures.md)) | an `AdwNavigationView`, a page per stage: its header bar's back button is Back, Cancel at the start, the primary a suggested action at the end | an `AdwCarousel` with `AdwCarouselIndicatorDots`, moved only by the buttons | a `GtkPopover` from a `GtkMenuButton` |

## Storage

| What | Where |
|---|---|
| The library: names, descriptions, notes, passages, order, whether each reveal was shown | `writing_styles.toml`, read once at launch and written behind each change, as `signatures.toml` is |
| Each account's slot; the endpoint's address, model and declaration; the jurisdiction mode | `preferences.toml`, the `[ai]` table |
| The endpoint's key | the platform keystore, id `ai-endpoint` |
| The `Message-ID`s of drafted replies | `drafted_replies.toml`, this device only: never capped, kept when a style is forgotten, gone with their account |

## Sovereignty scope

Learning and drafting **pass the jurisdiction gate**; neither is a carve-out
([`../AGENTS.md`](../AGENTS.md), "Non-negotiables"). The relay is `eu-native` by construction; an
own endpoint is admitted as far as the person's declaration and the mode allow, an undeclared one
only under `all`. A model on the person's own computer is declared like any other endpoint, so no
local or air-gapped deployment is exempt. Learning reads only the sent mail the device holds.

## Per-platform matrix

| Capability | Core | macOS | iOS/iPadOS | Windows | Android | Linux |
|---|:---:|:---:|:---:|:---:|:---:|:---:|
| The gate before anything is read or sent (rules 1 to 3) | 🚧 | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ |
| The three surfaces offered together, under early access (4, 5) | 🚧 | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ |
| The own endpoint: held fields, Save running, refusals said, the key in the keystore (6, 7) | 🚧 | ⬜ | ⬜ | 🚧 | ⬜ | ⬜ |
| Settings → Writing style from `writing_style_page`, Learn saying why (8 to 10) | 🚧 | ⬜ | ⬜ | 🚧 | ⬜ | ⬜ |
| Learning in the flow host, with Back, Stop and one remedy (11 to 14) | 🚧 | ⬜ | ⬜ | 🚧 | ⬜ | ⬜ |
| The reveal once, then the detail: rename, notes, Forget asked first (15, 16) | 🚧 | ⬜ | ⬜ | 🚧 | ⬜ | ⬜ |
| Draft a reply as the core says, the instruction in the platform's control, Stop (17 to 20) | 🚧 | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ |
| The draft as one undo step, replacing asked first (21, 22) | 🚧 | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ |
| The card, ticks from the editor's changes, asked once at Send (23, 24) | 🚧 | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ |
| A drafted reply never learned from: the log and the keyword on the Sent copy (25) | 🚧 | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ |
| The Allodia relay: its status, its refusals, the offer fed from `ai` (2, 5) | 🚧 | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ |

No platform ships the feature yet, and one ships it only once every row is ✅ there, each driven
against the harness's Sent Items and a canned endpoint, iPad and a production build signed in with
and without `ai` included: no cell turns on the strength of shared code alone.

Legend: ✅ shipped · 🚧 in progress, which is what a platform drawing a surface under early access is
([`settings.md`](settings.md), Writing style) · ⬜ planned · — not applicable.

## Enforcement

This contract is binding via [`../AGENTS.md`](../AGENTS.md). When you change what is learned, what
is sent, where it goes or how a draft enters the composer:

1. Update the rule and the matrix here in the same change, on every platform with the surface.
2. Pin each rule the core decides with a core test before a client draws it, and the bundle's
   insertion, its one undo step and the gaps it reports in `clients/composer/tests`.
3. Update [`settings.md`](settings.md), [`capabilities.md`](capabilities.md) and a changelog
   fragment when a person could notice, and [`privacy-policy.md`](privacy-policy.md), every locale
   moving with it, before any build sends mail text anywhere.
