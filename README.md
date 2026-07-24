<p align="center">
  <img src="assets/images/emilio.png" alt="Emilio" width="150">
</p>

<h1 align="center">Emilio</h1>

<p align="center"><em>The Diablo 2 Resurrected Best Buddy</em></p>

<p align="center">
  <a href="https://github.com/jmlemetayer/emilio/releases/latest"><img src="https://img.shields.io/github/v/release/jmlemetayer/emilio" alt="Latest release"></a>
  <a href="LICENSE.md"><img src="https://img.shields.io/github/license/jmlemetayer/emilio" alt="License: MIT"></a>
  <a href="https://github.com/jmlemetayer/emilio/commits/main"><img src="https://img.shields.io/github/last-commit/jmlemetayer/emilio" alt="Last commit"></a>
  <a href="https://github.com/jmlemetayer/emilio/actions/workflows/integration.yml"><img src="https://github.com/jmlemetayer/emilio/actions/workflows/integration.yml/badge.svg" alt="Continuous Integration"></a>
</p>

<br>

Emilio is a companion app for **Diablo II: Resurrected** (D2R), including the *Reign of the Warlock*
expansion, built for **Solo Self Found** (SSF) play. It runs locally on Windows, reads your own save
files, and keeps everything it learns on your machine.

I used to play with half a dozen things open at once: [MF Run Counter][mfrc] on one side of the
screen, [d2rHolyGrail][grail] on the other, and a browser full of tabs: [the Arreat Summit][arreat],
a [cheat sheet][cheatsheet], the [runeword explorer][runewords], the [terror zone calendar][tz]...
Each of them is good at its one job. None of them know about each other, and most of them need me
to tell them what I just did instead of noticing it themselves.

Emilio is my attempt at the single app I wanted instead: one window that watches the game's own save
files, works out when a run started and ended without being told, and knows what dropped because it
can read the save, not because I typed it in.

[mfrc]:       https://github.com/oskros/MF_run_counter
[grail]:      https://github.com/zeddicus-pl/d2rHolyGrail
[arreat]:     https://classic.battle.net/diablo2exp/
[cheatsheet]: https://d2r.guide
[runewords]:  https://d2runes.io/runewords
[tz]:         https://www.d2emu.com/tz-sp

**Status: pre-release, under active development.**

## Features

- [ ] **Automatic run tracking** — runs start and stop on their own, with no timer to press. Two
      modes, semi-auto and full-auto, described below.
- [ ] **Run history and stats** — every run logged with its duration and what it netted, tagged by
      run type, with per-tag totals, averages and bests.
- [ ] **Possession index and Holy Grail** — what you own right now and on which character or stash,
      ticked off against the grail automatically as you find it.
- [ ] **Character sheets** — attributes, skills, gear and one-time quest rewards, read straight from
      the save rather than typed in.
- [ ] **Item evaluation** — how good a roll actually is: a percentage of its possible range for
      uniques and sets, a letter grade for rares and crafts.
- [ ] **Smart loot alerts** — an overlay and a sound when something worth stopping for drops,
      including the rares no grail list tracks.
- **[And more...](TODO.md)**

## Semi-auto vs full-auto tracking

Emilio can time your runs two different ways. You pick which one in Settings.

**Semi-auto** is the default. It watches nothing but your own save files and whether the game is
running. Leaving a game, saving and quitting are all written to disk, so the end of a run is always
caught. The start of one is never written anywhere, so Emilio assumes the next run begins the moment
the last one ended. That holds up while you keep running, and a hotkey corrects it when it does not.

**Full-auto** reads D2R's memory instead, and never writes to it. This is the mode that knows
exactly when a game starts rather than assuming it, and it also sees when a game is paused, so time
spent sitting in the menu never counts towards a run. Nothing is guessed and no hotkeys are needed.
All it looks at is whether you are in a menu, in a game, or paused.

> [!WARNING]
> Reading game memory is how cheats work, even when timing runs is all you are doing with it.
> Blizzard's terms draw no distinction between the two, and Emilio has no way to prove its intent
> to an anti-cheat system. Enabling full-auto tracking may be treated as a violation and could get
> your account suspended or banned. It is off by default, it will stay off unless you turn it on,
> and the risk is yours to accept.

> [!NOTE]
> **If anyone from the D2R team ever reads this:** full-auto only exists because the start of a
> game cannot be detected by watching the save files. Every other moment of a run can. Any
> consistent write to a file when a game begins would be enough, and this entire mode could be
> deleted.

## Installation

Download the installer from the [latest release][latest] and run it. That is the whole
installation — Emilio is a single Windows application with no runtime to install alongside it.
Older versions stay available on the [releases page][releases].

Building from source is only needed if you intend to work on Emilio, and is covered in
[CONTRIBUTING.md](CONTRIBUTING.md).

[latest]:   https://github.com/jmlemetayer/emilio/releases/latest
[releases]: https://github.com/jmlemetayer/emilio/releases

## Usage

Start Emilio before or during a D2R session. It locates your save directory on its own, and you can
point it elsewhere in Settings.

From there it stays out of the way: the dashboard shows the run in progress, and the Runs screen
lists what you have done so far. In semi-auto mode, two hotkeys cover the moments Emilio cannot
see for itself:

| Hotkey       | Action                           |
| ------------ | -------------------------------- |
| `Alt+Q`      | Start or restart the current run |
| `Ctrl+Space` | Pause or resume the current run  |

Both are rebindable in Settings, and neither is needed in full-auto mode.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for git conventions, developer rules, and how to build from
source.

## Use of AI

AI-assisted contributions are allowed here, under exactly the same rules as every other
contribution.

This project exists because of AI. I had wanted to build a Diablo II companion app for years, and I
wanted to build it in Rust. Every previous attempt died the same way: I started, ran into the sheer
volume of work involved, hit a wall, and quietly forgot about it. Working with AI is what finally
got me past that wall.

So I am not going to turn around and refuse AI-assisted contributions — that would be hypocritical.
Use whatever tools you want. What I care about is the result, and the result is judged the same way
no matter who or what wrote it.

Every contribution must follow the rules in [CONTRIBUTING.md](CONTRIBUTING.md). They are written
for humans and coding agents alike, so if you work with an agent, make sure it follows them.
Whoever opens the pull request owns what is in it: you are expected to understand your own changes
and to stand behind them in review.

## License

Emilio is released under the [MIT License](LICENSE.md).
