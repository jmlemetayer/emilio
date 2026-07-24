# Contributing to Emilio

Everything here applies to humans and coding agents alike. If you are running an agent on this
repository, point it at this file first — it is the whole set of rules, and there is nothing
agent-specific hidden anywhere else.

If you only want to *use* Emilio, you do not need this page; see the [README](README.md).

## Getting set up

Requires a recent stable Rust toolchain (edition 2024).

```bash
git clone https://github.com/jmlemetayer/emilio.git
cd emilio
cargo build
```

Windows is the primary target and the only one currently tested.

## Project layout

- **`src/`** — the Emilio application: UI, settings, run engine, and anything else specific to this
  app.
- **`d2r/`** — reusable library: watching D2R, classifying save events, parsing saves.
- **`assets/`** — images and other files shipped with the application.

The dependency direction only ever points one way: `emilio` may depend on `d2r`, never the reverse.
When you are unsure where something goes, ask whether another D2R tool could use it — if yes, `d2r`.

## Code and comments

- **Make surgical changes.** Touch only what the change needs and preserve the style of the code
  around it. Unrelated reformatting buried in a functional change makes a diff far harder to review.
- **Comments are for what the code cannot say.** Write one only when names, types and log strings
  genuinely cannot convey what is happening, and comment the *what*.
- **Reasoning belongs in the commit message**, not in a comment block. Why it was done, what else
  was tried, which trade-off was accepted — that goes in the log, or in the docs if it is
  long-lived. A comment telling the whole story goes stale; the log entry never does.

## How changes land

Every change goes through a pull request, the maintainer's included. Nothing is committed straight
to `main`.

1. Branch off `main`. Short, hyphenated, descriptive: `process-tracker`, `fix-save-dir-detection`.
2. Commit as you work, following the convention below.
3. Open a pull request describing what changed and why.
4. CI must be green and review resolved before it merges.

Prefer new commits over rewriting history that has already been pushed.

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

Scale the message to the change. A trivial patch can be a subject line on its own; a subtle fix
earns the full analysis — what went wrong, how to reproduce it, why this fix and not another. The
diff already says what changed, so spend the message on what it cannot show.

Keep each commit to one logical change. A formatting sweep and a bug fix do not belong together.

[kernel]: https://www.kernel.org/doc/html/latest/process/submitting-patches.html

## Before you open a pull request

```bash
cargo fmt --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
```

All three must pass. CI runs the same checks, so anything failing here will fail there too.

`--workspace` is not decoration. The repository root is both a package and a workspace, so cargo
otherwise operates on the root package alone and silently skips every other crate — `cargo test`
without it runs none of `d2r`'s tests and still reports success.

## Hard rules

These get a change rejected however well it is written.

- **Nothing that confers a gameplay advantage.** Emilio reports on your own saves and on whether the
  game is running. It never surfaces monster data, item locations, map layouts, or anything that
  would change how you play. Where memory is read at all, it is read-only and limited to session
  state — menu, in game, or paused. See [the README](README.md#semi-auto-vs-full-auto-tracking).
- **Emilio only ever reads.** No writing to save files, no writing to game memory, no modifying the
  game installation.
- **No game assets in the repository.** Never commit art, sounds, or data files extracted from
  Diablo II: Resurrected. Emilio reads what it needs from the player's own installation at runtime;
  it does not redistribute Blizzard's content.
- **Tests never touch a real save directory.** Use fixtures or copies. A test pointed at a live save
  directory will not be merged, even if it only reads.

## Working with AI

Use whatever tools you like — this project was built with them and says so in the
[README](README.md#use-of-ai). There is nothing to declare and no separate process for AI-assisted
changes. What is expected is exactly what is expected of everyone else:

- **You own the pull request**, not the tool that wrote it. You are expected to understand every
  line you propose and to answer questions about it in review.
- **Read it before you send it.** A contribution that costs more to review than it saves to write is
  not a contribution.
- **Run the checks for real.** Never report a result you did not observe. If something is failing or
  unverified, say so in the pull request — that is useful, and it is checkable.
