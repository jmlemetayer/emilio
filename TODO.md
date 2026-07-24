# TODO

Everything planned for Emilio, grouped by area. The [README](README.md) lists the headline features;
this is the full list behind them, including the smaller things that will never get their own line
there.

Nothing here carries a date or a release number — items are ticked as they land, and order within
a section means nothing.

## Tracking & runs

- [ ] Persisted run history, so runs survive an app restart
- [ ] Retroactive run tagging (tag runs after the fact; per-tag stats recompute)
- [ ] Per-tag run stats & loot summary (count, total/average time, loot buckets)
- [ ] XP efficiency per run (XP/s, level-relative, compared against a personal baseline)
- [ ] Optional full-auto tracking via read-only memory access — session state only, see the
      [README](README.md#semi-auto-vs-full-auto-tracking)

## Items & possessions

- [ ] Live possession index across all characters and stashes (do I own X, how many, where)
- [ ] Full per-instance item parsing (affixes, sockets, quality, runewords)
- [ ] Roll quality for unique/set items (% of the known min-max range)
- [ ] Rare/crafted grading (S-E scale, build-aware)
- [ ] Build relevance matching (which build(s) an item is actually good for)
- [ ] Automatic Holy Grail tracking

## Characters

- [ ] Mule filtering (hide storage characters by default, with manual overrides)
- [ ] Character sheet: attributes, skills, gear, quest/one-time-resource usage
- [ ] Gear snapshots (named, timestamped, track where an item ends up later)
- [ ] Gold & gambling helper (gold vs. level cap, who's ready to gamble)

## Alerts & utilities

- [ ] Smart item alerts (overlay + sound on a good drop, delivered at the next save point)
- [ ] Additional system-wide hotkeys beyond start/pause
- [ ] Rune calculator (best High Rune reachable by cubing up from what's on hand)
- [ ] Terror zone calendar — which zone is terrorized right now, which one is up next, and the
      schedule further ahead
- [ ] Scratchpad notes

## Reference data

- [ ] Remote reference-data sync (item ranges, scoring, classification) with an offline fallback

## The app itself

- [ ] Automatic updates — spot a new release and install it, instead of downloading the installer
      by hand
