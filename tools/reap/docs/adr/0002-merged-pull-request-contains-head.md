# ADR 0002: a merged pull request that contains HEAD makes a worktree merged

- **Status:** Accepted
- **Date:** 2026-10-31
- **Amends:** [ADR 0001](0001-reap-sweeps-merged-worktrees.md), rules 3(c), 9 and 11

## Context

ADR 0001 rule 3(c) counts a squash- or rebase-merged worktree as merged only when `gh` reports
its pull request merged with a head SHA *equal* to the worktree's HEAD. Run over a busy
repository, that rule removed none of the worktrees it was meant for. Two gaps:

1. **The pull request moved on after the worktree stopped.** An agent pushes its branch, opens
   the pull request and stops; then a reviewer's fixer pushes to the same branch, or `main` is
   merged into it to clear a conflict. The pull request merges at a later head, and the local
   HEAD is an ancestor of it, never equal. Six of seven merged worktrees in one sweep were in
   this state.
2. **The pull request is too old for the list.** Rule 9 reads the 200 newest pull requests
   with one `gh pr list`. In a repository that opens dozens a day, a worktree whose pull request
   is a week old falls outside that window, so reap sees no pull request at all and the
   worktree stays "open" until it turns stale. Three of the same seven were outside it.

Both gaps can only *delay* a removal, never cause a wrong one, so nothing was lost. But the
pile-up reap exists to prevent grew back: over two weeks, five repos gathered 135 merged
worktrees.

## Decision

1. **Rule 3(c) becomes: `gh` reports the pull request merged, and HEAD is an ancestor of, or
   equal to, its merged head.** Every commit on the worktree is then in what merged, so this
   is as much a fact as equality was. reap checks it with `git merge-base --is-ancestor` when
   the merged head is in the clone, and otherwise asks GitHub's compare API
   (`repos/<owner>/<name>/compare/<HEAD>...<merged head>`), which answers `ahead` or `identical`
   only when HEAD is an ancestor. A HEAD that exists only locally, a failed call or any other
   answer is "not merged". reap still writes no ref other than the remote-tracking refs of rule
   8.
2. **Rule 11's `git branch -D` follows rule 3(c):** used when the branch tip equals HEAD and
   3(c) holds, since the merged pull request then holds every commit on the branch.
3. **Rule 9 looks up the pull requests of branches the list missed.** After the batched
   `gh pr list`, reap asks for the pull requests of every linked worktree's branch that is not
   among them, by head branch name, in one batched GraphQL call per pass (up to 100 branches
   a call). A branch with no pull request at all costs nothing more on the next pass than it
   did on this one.

Everything else in rules 3 to 6 stands, deliberately:

- **No tree or patch-id comparison** (ADR 0001, considered options). A worktree whose branch
  has no pull request, or whose HEAD is not in what merged, stays unmerged.
- **Untracked files still make a worktree dirty** (rule 4). An untracked file may be the only
  copy of something; archiving it somewhere and removing the worktree trades a visible pile-up
  for an invisible one.

## Considered options

- **Keep rule 3(c) as it was.** Safe, but in practice it removes almost nothing in the
  repositories with the most worktrees, which are the ones reap is for.
- **Fetch the merged head (`refs/pull/<n>/head`) and check ancestry locally.** No API call, but
  it writes refs outside `refs/remotes/`, which rule 8 rules out.
- **Raise the `gh pr list` limit.** Every pass then reads thousands of pull requests to find a
  handful, and the window still ends somewhere.
- **One `gh pr list --head <branch>` per missing branch.** Simple, but one call per branch per
  pass, every five minutes, spends the API rate limit that the operator's other tools share.
- **Tree comparison as a fallback** (every file the branch touched is identical on the base).
  It catches a branch with no pull request, but ADR 0001 rejected it as a guess, and nothing
  here changes that.

## Consequences

- A worktree whose pull request merged after more commits were pushed to it is removed after
  the grace period, as ADR 0001 intended.
- A pass costs one more GraphQL call per repo when some worktree branches are not among the 200
  newest pull requests, and one compare call for each merged worktree whose merged head is not
  in the clone. The compare calls stop once the worktree is removed.
- A worktree whose branch was renamed, or whose pull request came from a fork, still has no
  pull request reap can find, and stays unmerged.
