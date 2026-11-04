# CLAUDE.md

git-reap (`git reap`) removes the git worktrees whose work has merged, from a macOS menu-bar
app and a CLI. Spec: [ADR 0001](docs/adr/0001-reap-sweeps-merged-worktrees.md).

Part of steading: the repo-wide commands, commit rules and owner paths are in the
[root CLAUDE.md](../../CLAUDE.md). Paths and commands below are relative to this folder
(`tools/reap/`).

## Commands

- `sh -n install.sh` and `./install.sh --dry-run` — installer syntax and dry run (the steps live in
  `../../scripts/install-app.sh`); never run it without `--dry-run` while developing
- `scripts/render-icons.sh` — the app and tray icons from `app/icons/src/*.svg`; commit sources and PNGs together
- `npm ci` at the repository root (never `npm install` inside `app/ui`), then `npm --prefix app/ui run typecheck`, `npm --prefix app/ui test -- --run`, `npm --prefix app/ui run build` — the popover
  ([styleguide](docs/styleguide.md))
- `npm --prefix app/ui run storybook` — every popover state as a story, on port 6007, dark and light
  from the toolbar

## Safety rules for code

reap deletes things, so ADR 0001 rules 3 to 6 and 11 are invariants, not defaults: removal is
`git worktree remove` without `--force`, `git branch -D` only with `gh`'s proof that a merged
pull request contains the tip (ADR 0002), nothing on a remote, nothing in a main clone. The one
`--force` is a dirty merged worktree's, and only after its changes are saved to
`refs/reap/salvage/` and checked
([ADR dirty-merged-worktrees-are-salvaged](docs/adr/0004-dirty-merged-worktrees-are-salvaged.md));
`git stash` is never used. A change
that loosens one of them needs a new ADR first. Tests build real repositories in a temp dir; they
never touch a clone outside it, and never reach GitHub (a fake `gh` answers from fixtures, rule 17).
