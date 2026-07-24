# Contributing to Emilio

Everything here applies to humans and coding agents alike. If you are running an agent on this
repository, point it at this file first — it is the whole set of rules, and there is nothing
agent-specific hidden anywhere else.

If you only want to *use* Emilio, you do not need this page; see the [README](README.md).

## Getting set up

Both crates are edition 2024, so Rust 1.85 or later. CI runs current stable and nothing else.

```bash
git clone https://github.com/jmlemetayer/emilio.git
cd emilio
cargo build
```

Windows is the only platform Emilio is meant to run on, and the only one CI tests, though the checks
below pass anywhere.

## Project layout

- **`src/`** — the Emilio application: UI, settings, run engine, and anything else specific to this
  app.
- **`d2r/`** — reusable library: watching D2R, classifying save events, parsing saves.
- **`d2r/examples/`** — small example programs, to showcase or exercise part of the library.
- **`assets/`** — images and other files shipped with the application.

The dependency direction only ever points one way: `emilio` may depend on `d2r`, never the reverse.
When you are unsure where something goes, ask whether another D2R tool could use it — if yes, `d2r`.

## Code and comments

- **Make surgical changes.** Touch only what the change needs and preserve the style of the code
  around it. Unrelated reformatting buried in a functional change makes a diff far harder to review.
- **Comments are for what the code cannot say.** Write one only when names, types and log strings
  genuinely cannot convey what is happening, and comment the *what*.
- **Module documentation carries the design; the commit message carries the change.** Why the code
  is shaped as it is goes in the module documentation, where the next reader will actually find it.
  Why this particular change happened, what else was tried and which trade-off was accepted goes in
  the commit message. Neither belongs in a comment block inside a function, which is where it goes
  stale.
- **Public items get a doc comment** saying what they are for and any assumption a caller could get
  wrong.

## How changes land

Every change goes through a pull request, the maintainer's included. Nothing is committed straight
to `main`.

1. Branch off `main`. Short, hyphenated, descriptive: `process-tracker`, `fix-save-dir-detection`.
2. Commit as you work, following the convention below.
3. Open a pull request describing what changed and why.
4. CI must be green and review resolved before it merges.

## Commit messages

Emilio follows the [kernel guidelines][kernel].

```
<scope>: <imperative summary>

Describe the problem first, then the fix — the why and the how. Wrap the
body at around 75 columns. Add trailers such as Fixes: when relevant.
```

`<scope>` is the crate or area touched — `d2r`, `emilio`, `docs`, `ci`, `build`. The summary is
imperative and lower case with no trailing period: *add a process tracker*, not *Added a process
tracker.*

**Tell it as a story, in two or three short paragraphs.** Order them the way the diff raises its
questions — what the problem was, what was done about it, and why that way rather than the obvious
way — not the way you worked the change out. That is the size of a substantial change, not a quota:
a trivial patch is a subject line on its own, and a longer message starts explaining what the diff
already shows, or gets skipped whole. Ask of every sentence which line of the diff it helps read, and
cut it when the answer is none — that catches both a sentence restating the diff and one defending
the change against an alternative nobody reading it would have thought of.

**Name what is in the diff.** A dependency, a file or a function the reader has in front of them
gets called by its name, not *the crate* or *the new module*. The reverse holds for anything the diff
does not contain: a name tied to nothing on screen just sends the reader hunting, so where one has to
be mentioned, assert it in a clause and move on.

**Plain text, no markup.** A commit message is read as-is in a terminal, not rendered as Markdown.
Write an identifier plain rather than wrapped in backticks, and avoid the em dash and other
non-ASCII punctuation: a comma, colon or parenthesis says the same thing in characters every
terminal renders the same way.

**Prioritise plain sentences over short ones.** A sentence that has to be read twice has failed,
however few words it spends, and compression is the usual reason: a pronoun standing in for a noun
the reader has to hunt for, two ideas folded into one sentence, a clause parked where it attaches to
the wrong verb. Say the subject out loud, keep one idea per sentence, and resist the epigram.

**Read it back before you commit:** check that the subject and first sentence say what landed, then
point each remaining sentence at the hunk it serves. Do it again after an amend or a rebase, because
a message that was right when written goes stale as soon as hunks move to another commit.

Keep each commit to one logical change. A formatting sweep and a bug fix do not belong together.
Prefer new commits over rewriting history that has already been pushed.

[kernel]: https://www.kernel.org/doc/html/latest/process/submitting-patches.html

## Before you open a pull request

```bash
cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
```

All three must pass. The CI workflow runs these same three commands and is the authority: where it
disagrees with your machine, it is right.

> [!NOTE]
> `--all` and `--workspace` are not decoration. The repository root is both a package and a
> workspace, so cargo otherwise operates on the root package alone and silently skips every other
> crate — `cargo test` without `--workspace` runs none of `d2r`'s tests and still reports success.
> `cargo fmt` is the one that looks safe without it: it reaches `d2r` today only because `emilio`
> depends on `d2r`, and would stop the moment a crate joins the workspace without being a
> dependency.

## Hard rules

These get a change rejected however well it is written.

- **Nothing that confers a gameplay advantage.** Emilio reports on your own saves and on whether the
  game is running. It never surfaces monster data, item locations, map layouts, or anything that
  would change how you play. Where memory is read at all, it says nothing beyond whether the player
  is in a menu, in a game, or paused. See [the README](README.md#semi-auto-vs-full-auto-tracking).
- **Emilio only ever reads.** No writing to save files, no writing to game memory, no modifying the
  game installation.
- **No game assets in the repository.** Never commit art, sounds, or data files extracted from
  Diablo II: Resurrected. Emilio reads what it needs from the player's own installation at runtime;
  it does not redistribute Blizzard's content.
- **Tests never touch a real save directory, or the filesystem at all.** Assemble the situation out
  of made-up events instead. A test pointed at a live save directory will not be merged, even if it
  only reads.

## Working with AI

Use whatever tools you like — this project was built with them and says so in the
[README](README.md#use-of-ai). What is expected is exactly what is expected of everyone else:

- **You own the pull request**, not the tool that wrote it. You are expected to understand every
  line you propose and to answer questions about it in review.
- **Read it before you send it.** A contribution that costs more to review than it saves to write is
  not a contribution.
- **Run the checks for real.** Never report a result you did not observe. If something is failing or
  unverified, say so in the pull request — that is useful, and it is checkable.
