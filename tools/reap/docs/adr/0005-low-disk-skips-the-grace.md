# ADR 0005: on a low disk, removable worktrees go without waiting out their grace

- **Status:** Accepted
- **Date:** 2026-11-05
- **Amends:** [ADR 0001](0001-reap-sweeps-merged-worktrees.md), rule 10

## Context

reap already sweeps every `interval`, so it runs when the disk is low. What it cannot do then is
free space: a removable worktree waits out its grace (24 h by default) after reap first sees it,
however full the disk gets. A day of agent work can make several worktrees of a few gigabytes
each removable. When the disk fills, the build that needs the space fails, and the operator
removes the worktrees by hand, which is what reap exists to prevent.

The grace exists so the operator hears about a removal before it happens and can *Keep* the
worktree. On a nearly full disk that warning matters less than the space. reap already measures
what each removal frees (`disk_usage`), so it can pick the removals that free the most.

## Decision

1. **Low disk is per volume.** A volume is low when the space a non-root process can write
   (`statfs`'s available blocks) is below `low_disk` of its size. `low_disk` is a percentage
   (default `"10%"`), a size (`"20GB"`), or `"off"`, set globally and per repo. Each worktree
   counts on the volume its folder is on, so a worktree on an external drive counts there. Every
   tick of the tray and every `sweep --due` reads the free space of each volume that holds a
   listed repo's worktrees, whether or not those repos are due for a pass.
2. **A low volume skips the grace for removable worktrees only.** A worktree on a low volume
   that is removable (merged under ADR 0001 rule 3, or pushed and stale under ADR 0003), in a
   repo with `policy = "auto"` and `low_disk` not `"off"`, is removed without waiting for its
   grace clock. Everything else stays as it is:
   - dirty worktrees keep their `dirty_grace` and the salvage rules of
     [ADR dirty-merged-worktrees-are-salvaged](0004-dirty-merged-worktrees-are-salvaged.md);
   - kept, in-use and locked worktrees are not removed;
   - `policy = "ask"` sends its notification right away, marked *low disk*, and still removes
     only on confirm; `policy = "off"` removes nothing;
   - missing folders keep rule 12's grace, since pruning them frees no space.

   ADR 0001 rules 3 to 6 and 11 still apply. Each removal runs in a pass under the repo's lock
   that checks the worktree again, and a repo that tether holds is skipped as in rule 7.
3. **Largest first, until healthy.** A pressure pass takes the removable worktrees on the low
   volume from every listed repo and orders them by measured size, largest first. After each
   removal it reads the volume's free space again and stops once that is at least 1.5 times
   `low_disk` (15% free with the default). It reads the volume instead of trusting the
   measurement, since APFS clones count in full in `disk_usage`. Worktrees it does not reach keep
   their grace clocks.
4. **The notification comes after the removal.** A pressure pass sends one notification per
   volume: *Low disk on <volume>: removed N worktrees, freed X*, naming each one. When it removes
   nothing and the volume is still low, it says so once per episode, with what holds the space
   (dirty, kept, in use, with their sizes). An episode ends when the volume is healthy again. The
   tray shows *needs you* while a volume stays low and reap has nothing left to remove there.
5. **CLI and tests.** `git reap status` shows each volume's free space and threshold, and marks
   a low one. `sweep --due` runs pressure passes; `--dry-run` prints them as `would run:` lines.
   The free-space reading is injected into the core, as the clock and `gh` are (rule 17), so
   tests simulate a low volume without filling a disk.

## Considered options

- **Also skip `dirty_grace`.** It frees the most, but it salvages and removes a worktree whose
  changes may be from the last hour. The owner kept salvage on its own clock (2026-11-05).
- **Only notify.** It needs no change to rule 10, but the operator then does the removal reap
  could have done.
- **Remove everything removable at once.** Simpler, but it throws away the warning for
  worktrees whose removal the disk did not need.
- **A fixed size as the default, such as 20 GB.** It triggers too early on a small disk and too
  late on a large one. A size stays available as a setting.
- **macOS's "available for important usage" capacity.** It counts purgeable space as free, which
  matches Finder, but it needs Foundation in a core that is UI-free and builds on Linux.
- **Delete build output (`target/`, `node_modules/`) inside worktrees instead.** It frees space
  without removing anything the operator might return to, but reap would then write inside a
  worktree, which no rule allows. That would need its own ADR.

## Consequences

- On a low disk, a removable worktree can be removed without the warning ADR 0001 rule 10
  promises. The notification arrives afterwards. *Keep*, `policy = "ask"` or `"off"`, and
  `low_disk = "off"` still prevent it.
- Space held by dirty, kept or in-use worktrees is not freed. The notification names them, so
  the operator knows what to remove.
- `statfs` counts purgeable APFS space (local snapshots, caches) as used, so reap can act while
  Finder shows more free space than the threshold. `low_disk` sets the level.
