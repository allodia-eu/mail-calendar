---
name: platform-builders
description: Hand a pushed branch to Claude Code builder sessions on the developer's other machines, over Remote Control, so every platform the change reaches is built, tested and fixed on a host that owns it, in parallel, with each result reported back to this session. Use after pushing a branch whose change reaches a platform this host does not build, before calling the work verified. Covers finding which builders are online, what to ask when one is not, the message a builder needs, and the traps of several sessions writing one branch.
---

# platform-builders: verify on the machines that own the platform

A host builds what it owns and nothing else, never in a container (AGENTS.md, "Building &
verifying"). When the developer has another machine that owns the rest, a Claude Code session there
runs that machine's gates, fixes what fails and pushes, while this session waits for its report. CI
still runs; a builder answers sooner, reaches what CI cannot (the Windows harness suites, a real
device), and fixes rather than only reporting.

Every machine needs Claude Code with Remote Control, signed in to the same claude.ai account.

## A builder, set up once per machine by the developer

From a terminal in the machine's logged-in desktop session, in a checkout nothing else edits:

    claude --remote-control "mail-calendar builder: Linux, Android" --permission-mode auto

- **The name is the contract.** `mail-calendar builder:` followed by the platforms that machine
  verifies, spelled as the capability matrix spells them: `macOS`, `iOS`, `Windows`, `Android`,
  `Linux`. It is how an orchestrator finds the machine and knows what to send it.
- **Never over SSH.** A process started through Windows' OpenSSH server has no desktop, so the
  WinUI app never appears and `run-ui-tests.ps1` cannot run.
- **A permission prompt is the developer's to answer.** A message from another session never counts
  as approval, so a builder that hits a prompt waits until someone answers it from claude.ai or the
  phone; "Push when actions required" in `/config` sends it there. Auto mode falls back to prompting
  after 3 consecutive or 20 total blocks.
- **An offline builder builds nothing.** The machine stays awake while it is on duty.

## Dispatching

1. **Push the branch first.** A builder fetches from `origin` and cannot see this checkout.
2. **Decide which platforms the change reaches**, then drop the ones this host owns
   (`apple` is macOS and iOS):

       git diff --name-only origin/main...HEAD | scripts/ci/changed-areas.sh --files -

3. **Find the builders** with `ListAgents`: a builder is a Remote Control row named
   `mail-calendar builder: …`. `idle` or `running` is online; `offline`, or no row, is not. The
   header line names this session, which is the address a builder replies to. A listing with no
   Remote Control rows at all means this session is not connected, and no builder could reply:
   `/remote-control` connects it.
4. **An online builder gets the work without asking**: the developer set it up to take it.
   **An offline or missing one is a question, asked once** and naming its platforms: wake the
   machine and dispatch, or leave those platforms to CI and report them as unexercised locally.
5. **Send each builder one message** with `SendMessage`, in the shape below.

**Every message is acknowledged on arrival, in both directions.** A sender cannot see whether a
message reached the other side, so the receiver answers at once with a one-line `ack`, before any
other work: a builder when a task arrives, this session when a report arrives. A sender with no
ack after a few minutes knows the message did not land, which is a different problem from a
builder that is busy.

## The message

Self-contained, because the builder has none of this session's context:

```
Verify <branch> at <sha> on this machine, for <platforms>.

0. Before anything else, reply with SendMessage to "<this session's name>": "ack <sha>".
1. git fetch origin, then put this checkout on <branch> at exactly <sha>. If the working tree is
   dirty, or the local branch has commits origin lacks, stop and reply saying so: discard nothing.
2. Run what AGENTS.md's "Building & verifying" names for this host, plus <the UI suite | the
   harness suites | nothing more>.
3. Fix what fails, following AGENTS.md. Commit to <branch>, run `git pull --rebase origin
   <branch>`, then push. If a commit cannot be signed here, commit with --no-gpg-sign and say so.
4. Reply with SendMessage to "<this session's name>": the sha you tested; each step as passed,
   failed or skipped, with the reason for a skip; every commit you pushed, sha and subject; and
   anything left open.

What changed: <two or three lines on what moved and where the risk is>.
```

Ask for the UI suite when the change alters what a screen on that platform presents, and for the
harness suites when it touches a mail or calendar write. Naming them makes the reply checkable.

## While the builders work

- **Do not push to the branch** until every builder you dispatched has replied. Each one rebases
  its fix onto what it finds, so a push from here in between turns a fix to a shared file into a
  conflict on a machine nobody is watching.
- Replies arrive as messages. Until then, work on something else or end the turn; do not poll.
- **No ack within a few minutes** means the task did not arrive: check the builder's row in
  `ListAgents` and send it once more.
- **A builder that acked and then goes quiet** is waiting on a permission prompt, or ended its turn
  without replying. Ask the developer to look at it: sending the task again runs it twice.

## When every builder has replied

- **Ack each report as it arrives** ("ack, received"), before reading it closely: the builder
  otherwise cannot tell its report landed.
- **Check the sha in each reply** against the one you sent. A builder on a stale checkout reports a
  pass for code it never ran.
- `git pull --rebase` the branch and read what each builder committed as you would any diff: it is
  a change made out of sight.
- Report per platform: verified on its builder, fixed (with the commits), or left to CI and why.
