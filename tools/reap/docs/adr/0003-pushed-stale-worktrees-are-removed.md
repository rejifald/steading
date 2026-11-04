# ADR 0003: a stale worktree whose commits are all pushed is removed, its branch kept

- **Status:** Accepted; rule 3 amended by
  [ADR 0006](0006-abandoned-worktrees-are-saved-then-offered.md)
- **Date:** 2026-10-31
- **Amends:** [ADR 0001](0001-reap-sweeps-merged-worktrees.md), rule 13

## Context

ADR 0001 rule 13 flags a stale worktree and never removes it: its pull request closed
without merging, or nothing has happened in it for `stale_after`. The operator decides what
to do with it. In practice most of them need no decision. A worktree whose branch is on the
remote, with a clean tree, holds nothing that the remote and its local branch do not already
hold. Its checkout and build output take disk space and a *Needs you* row, and the operator's
only move is to remove it. Of thirteen stale worktrees across one operator's repos, three
were exactly that.

## Decision

1. **A stale worktree is removable when every commit on it is on the remote** (no commit on
   HEAD outside `refs/remotes/<remote>/*`), it is on a branch, and it has no open pull
   request. Rules 4 to 6 and 10 apply unchanged: clean, not in use, not locked, not kept, and
   removed after the grace period under the repo's `policy`.
2. **Its branch is kept.** reap removes the worktree and never deletes the branch. The
   branch then holds the commits even if the remote branch is deleted later, or the
   remote-tracking ref was already stale when reap looked. Removing a clean worktree whose
   branch stays loses nothing but ignored files, the same as a merged removal (rule 4).
3. **A guard that holds it leaves it stale.** A pushed stale worktree that is dirty, in use
   or locked stays in *Needs you* with its stale reason. One whose branch has commits the
   remote lacks, or a detached HEAD, stays there too.

## Considered options

- **Keep rule 13 as it was.** It is the safest option, but the operator then removes by hand
  what reap could have removed, which is what reap exists to prevent.
- **Delete the branch too.** That frees nothing measurable. Without a fresh `ls-remote`, it
  can also lose work when the remote branch has gone and its tracking ref is stale.
- **Include detached worktrees.** A detached HEAD has no branch to keep, so its commits
  would rest on the remote alone.
- **Include worktrees with an open pull request.** An open pull request is work someone may
  come back to, even after weeks. Its worktree stays until the pull request merges or closes.
- **Require a fresh `ls-remote` for each branch.** Not needed while the branch is kept.

## Consequences

- Abandoned but pushed worktrees leave *Needs you* and are removed a grace period later, with
  the same notification as a merged one. Their branches stay in the clone.
- *Needs you* now holds only the stale worktrees with something local: unpushed commits,
  uncommitted changes, or a detached HEAD.
- The grace notification is titled "worktree to remove", not "worktree merged", since it now
  covers both cases.
