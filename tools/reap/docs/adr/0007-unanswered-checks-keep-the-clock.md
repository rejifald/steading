# ADR 0007: a check that cannot be made keeps the grace clock, and a pressure pass judges each repo once

- **Status:** Proposed
- **Date:** 2026-11-19
- **Amends:** [ADR 0001](0001-reap-sweeps-merged-worktrees.md), rule 10; and
  [ADR low-disk-skips-the-grace](0005-low-disk-skips-the-grace.md), rule 2

## Context

On one operator's Mac, reap showed merged worktrees as *removes at …* for days and never removed
them. Two causes:

1. **A failed question to GitHub restarts the clock.** A squash-merged worktree is merged only by
   rule 3(c) or 3(d): GitHub has to say so. Every way of asking (`gh pr list`, the GraphQL head
   and commit lookups, `gh api …/compare`) read a failure as "not merged". So one pass made while
   the Mac was offline after sleep, rate limited or signed out turned every such worktree *open*,
   and rule 10 ("restarts if the worktree stops being removable") dropped its clock. The state
   file showed some 25 clocks of one repo restarted in the same minute. A repo with a failed call
   at least once a day never removes anything. A failed `lsof` (rule 5: "a guard that cannot be
   checked does not pass") drops a merged worktree's clock the same way.
2. **A pressure pass inspects the whole repo once per removal.** ADR low-disk-skips-the-grace
   runs each removal "in a pass under the repo's lock that checks the worktree again". It did
   that by inspecting the whole repo for every candidate: `lsof`, `git status` in every
   worktree, and the GitHub calls. With 84 worktrees and 33 candidates in one repo, a dry run
   took over an hour, and each tick of the tray on a low disk did the same.

## Decision

1. **A check that cannot be made is not an answer.** When a question to GitHub fails in a pass
   (anything but GitHub saying it does not have the commit), a worktree it left unmerged is
   *undecided*. So is a worktree whose processes `lsof` could not check. An undecided worktree is
   still not removable, and nothing is removed on its account, but a grace clock it already has at
   its current HEAD is kept, not restarted. Rule 10's "restarts if the worktree stops being
   removable" now reads "restarts if the worktree is found not removable". New commits still end
   a clock.
2. **`status` says so.** A repo with a failed GitHub question in its pass carries a note that the
   worktrees it could not judge keep their grace clocks.
3. **A pressure pass judges each repo once.** When the first candidate of a repo comes up, the
   pass takes that repo's lock and inspects it, and keeps both until the volume is done. Every
   candidate of that repo is judged against that inspection, which still runs every guard of
   ADR 0001 rules 3 to 6 and 11 under the lock. Before the second and later removals in a repo,
   `lsof` is read once more, and a worktree with a process started inside it since is skipped, as
   a normal pass does before its removals.

## Considered options

- **Retry the failed call.** It helps with a blip, but not with an hour offline or a rate limit,
  and it slows every pass further.
- **Keep every clock until HEAD moves.** Simpler, but then a worktree a real guard holds (dirty,
  locked, in use) would be removed the moment the guard lifts, without the warning rule 10
  promises. Only checks that could not be made keep the clock.
- **Cache GitHub's compare answers in `state.json`.** They never change for a pair of commits, so
  they could be kept across passes and save most calls. It is worth doing, but it is a separate
  change from not losing clocks.
- **Inspect only the candidate's worktree under the lock.** Cheaper still, but `inspect` judges a
  repo's worktrees together (one `gh pr list`, one lookup for every unmerged HEAD), so it would
  need its own path through the guards. One inspection per repo already makes the cost fixed.

## Consequences

- A merged worktree is removed when its first grace is up, even if some passes in between could
  not reach GitHub. It is still removed only by a pass that finds it removable.
- A failed GitHub question is visible in `status` and the popover instead of only delaying
  removals.
- A pressure pass holds a repo's lock from its first removal until the volume is done, so another
  run (a `sweep`, *Remove now*) waits or skips that repo for that long.
- On a low disk with many candidates, a pass costs one inspection per repo plus one `lsof`, not
  one inspection per candidate.
