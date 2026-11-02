# CLAUDE.md

git-reap (`git reap`) removes the git worktrees whose work has merged, from a macOS menu-bar
app and a CLI. Spec: [ADR 0001](docs/adr/0001-reap-sweeps-merged-worktrees.md).

Part of steading: the repo-wide commands, commit rules and owner paths are in the
[root CLAUDE.md](../../CLAUDE.md). Paths and commands below are relative to this folder
(`tools/reap/`).

## Commands

- `scripts/render-icons.sh` — the app and tray icons from `app/icons/src/*.svg`; commit sources and PNGs together
- `npm --prefix app/ui ci`, then `run typecheck`, `test -- --run`, `run build` — the popover
  ([styleguide](docs/styleguide.md))

## Safety rules for code

reap deletes things, so ADR 0001 rules 3 to 6 and 11 are invariants, not defaults: removal is
`git worktree remove` without `--force`, `git branch -D` only with `gh`'s proof that a merged
pull request contains the tip (ADR 0002), nothing on a remote, nothing in a main clone. A change
that loosens one of them needs a new ADR first. Tests build real repositories in a temp dir; they
never touch a clone outside it, and never reach GitHub (a fake `gh` answers from fixtures, rule 17).
