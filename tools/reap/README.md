# git-reap

Run as `git reap`.

Removes the git worktrees whose work has merged. You list your main clones; reap finds
their linked worktrees, shows each one with its pull request and CI, and removes a worktree
once its work has merged, its tree is clean and nothing runs inside it, after a grace period
(24 h by default) with a notification and a *Keep* button. An abandoned worktree whose commits
are all pushed goes the same way, with its branch kept
([ADR 0003](docs/adr/0003-pushed-stale-worktrees-are-removed.md)). A merged worktree with
uncommitted changes goes too, once nothing in it has changed for a week: its changes are saved
as one commit under `refs/reap/salvage/` in the main clone first, checked, and kept for 30 days
([ADR dirty-merged-worktrees-are-salvaged](docs/adr/0004-dirty-merged-worktrees-are-salvaged.md)).
It never touches a main clone's checkout or a remote, and never removes a worktree that is
locked, in use, mid-merge or mid-rebase, or holds commits only it has.

The spec is [ADR 0001](docs/adr/0001-reap-sweeps-merged-worktrees.md); rule numbers in the
code and docs refer to it. It is a companion to
[git-tether](https://github.com/rejifald/git-tether), which keeps listed clones on their
branch tip, and has the same shape: one binary that is a macOS menu-bar app without a
subcommand and the CLI with one.

**Status:** sweeps. A pass fetches what moved, runs the grace clocks, and removes merged
worktrees with git's own guards (rules 2 to 13); `git reap status` and the menu-bar app show
every worktree with its pull request and reap state. The popover's design is a first cut, and
there is no installer yet.

| Path | What |
|---|---|
| `core/` | `reap-core`: config, merge detection, the report and the `git-reap` CLI. No UI deps. |
| `app/` | `reap-app`: the Tauri tray shell; `app/ui/` is the React popover ([styleguide](docs/styleguide.md)). |
| `docs/adr/` | Decisions; [0001](docs/adr/0001-reap-sweeps-merged-worktrees.md) is the spec. |
| `scripts/check-sensitive.sh` | Keeps local paths, secrets and identities out of commits ([CLAUDE.md](CLAUDE.md) § Commits). |

## Quickstart

Needs a Rust toolchain (1.90 or newer) and git; `lefthook` for the hooks
(`brew install lefthook`).

```bash
lefthook install
```

```bash
cargo test --workspace --locked
```

Run the CLI against your own clones. `status` and `sweep --dry-run` only read them; `add`
writes `~/.config/reap/repos.toml`; a `sweep` without `--dry-run` removes worktrees whose grace
period has passed:

```bash
cargo run -q -p reap-core -- add ~/Development/app
```

```bash
cargo run -q -p reap-core -- status
```

```bash
cargo run -q -p reap-core -- sweep --dry-run
```

Run the menu-bar app (needs Node for the popover):

```bash
npm --prefix ../.. ci && npm --prefix app/ui run build && cargo run -p reap-app
```

Preview the popover in a browser against fake data: `npm --prefix app/ui run dev` (the
`reap-ui` entry in `.claude/launch.json`), at 360 px wide.

## Commands

| What | Command |
|---|---|
| Test | `cargo test --workspace --locked` |
| Lint | `cargo clippy --workspace --all-targets --locked -- -D warnings` |
| Format check | `cargo fmt --all --check` |
| Sensitive-info check | `scripts/check-sensitive.sh --all` |
| UI typecheck / tests / build | `npm --prefix app/ui run typecheck`, `npm --prefix app/ui test -- --run`, `npm --prefix app/ui run build` |
| Render icons from `app/icons/src/*.svg` | `scripts/render-icons.sh` |

The same checks run in the git hooks (`lefthook.yml`) and in CI (`.github/workflows/ci.yml`).

## Configuration

`~/.config/reap/repos.toml` lists the main clones (rule 1): `path`, optional `remote`
(default `origin`) and `base` (default the remote's default branch), and `interval` (default
`5m`), `policy`, `grace` and `stale_after` (default `14d`) globally and per repo (rules 7, 10,
13). Three more settings, also global and per repo, govern merged worktrees with uncommitted
changes: `dirty_grace` (default `7d`, how long nothing in it may change before its changes are
saved), `salvage_max_untracked` (default `100MB`, the most untracked data a save copies) and
`salvage_max_age` (default `30d`, when a sweep deletes a save). `~/.config/reap/state.json`
holds the grace clocks, *Keep*s and refused removals; a *Pause* file lives beside both.

## Salvaged changes

A merged worktree with uncommitted changes is saved before it is removed: `git add -A` into a
copy of its index, `write-tree`, `commit-tree -p HEAD`, then a ref
`refs/reap/salvage/<folder>/<YYYYMMDD-HHMMSS>` in the main clone. reap removes the worktree
with `--force` only after checking that the commit holds every change and untracked file
(ignored files are dropped, as in any removal). It holds the worktree instead when a merge,
rebase, cherry-pick or revert is under way, paths are unmerged, an untracked folder is a
repository of its own, or the untracked files are over `salvage_max_untracked`; its row says
which.

`git reap salvaged` lists the saves (`--json` too), and `git reap status` and the popover show
them under each repo. Each one comes with the commands that restore it, for example:

```bash
git -C ~/Development/app worktree add ~/Development/app-feat refs/reap/salvage/app-feat/20261103-231018 && git -C ~/Development/app-feat reset HEAD~1
```

The worktree comes back detached at the saved commit; `reset HEAD~1` undoes that commit, so
the changes are uncommitted again (all unstaged: the staged/unstaged split is not kept).

| Variable | What |
|---|---|
| `REAP_CONFIG_DIR` | the config folder instead of `~/.config/reap` |
| `REAP_GIT` | the `git` binary instead of `git` on `PATH` |
| `REAP_GH` | the `gh` binary instead of `gh` on `PATH`; tests point it at a fake |

## Architecture

ADR 0001 rules 14 to 18: a Rust core crate, a Tauri v2 tray shell with a React popover, and
the CLI in the same binary, linked as `git-reap`. The shell is copied from git-tether: one
worker thread runs git and sweeps every interval and when the popover opens, the event loop
never runs git, and the popover receives the same JSON that `git reap status --json` prints.

The menu-bar icon is the mark, four cubes with the back one lifted out, in one of three
states: *tidy* is the mark, *removal pending* is the mark in outline, *needs you* is the mark
in orange. The first two are template images, so macOS tints them for the menu bar.
`scripts/generate-mark.mjs` draws every icon from one geometry, and `scripts/render-icons.sh`
renders them; [docs/brand](docs/brand/README.md) has the meaning, geometry and colours.
