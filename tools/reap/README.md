# git-reap

Run as `git reap`.

Removes the git worktrees whose work has merged. You list your main clones; reap finds
their linked worktrees, shows each one with its pull request and CI, and removes a worktree
once its work has merged, its tree is clean and nothing runs inside it, after a grace period
(24 h by default) with a notification and a *Keep* button. An abandoned worktree whose commits
are all pushed goes the same way, with its branch kept
([ADR 0003](docs/adr/0003-pushed-stale-worktrees-are-removed.md)). It never touches a main
clone or a remote, and never removes a worktree that is dirty, locked, in use, or holds
commits only it has.

The spec is [ADR 0001](docs/adr/0001-reap-sweeps-merged-worktrees.md); rule numbers in the
code and docs refer to it. It is a companion to
[git-tether](https://github.com/rejifald/git-tether), which keeps listed clones on their
branch tip, and has the same shape: one binary that is a macOS menu-bar app without a
subcommand and the CLI with one.

**Status:** sweeps. A pass fetches what moved, runs the grace clocks, and removes merged
worktrees with git's own guards (rules 2 to 13); `git reap status` and the menu-bar app show
every worktree with its pull request and reap state. The popover's design is a first cut.

| Path | What |
|---|---|
| `core/` | `reap-core`: config, merge detection, the report and the `git-reap` CLI. No UI deps. |
| `app/` | `reap-app`: the Tauri tray shell; `app/ui/` is the React popover ([styleguide](docs/styleguide.md)). |
| `install.sh` | Build, install to `~/Applications`, link the CLI as `git-reap`, login item; `--uninstall` removes them. The steps are the repo's [`scripts/install-app.sh`](../../scripts/install-app.sh). |
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
npm --prefix app/ui ci && npm --prefix app/ui run build && cargo run -p reap-app
```

Install it (rule 16): build, show `git reap status` and the first `sweep --dry-run`, ask, then
copy to `~/Applications/reap.app`, link `~/.local/bin/git-reap`, and register and start the login
item `~/Library/LaunchAgents/io.github.rejifald.reap.plist`. Answering no installs nothing.

```bash
./install.sh --dry-run
```

```bash
./install.sh
```

`--uninstall` removes the login item, the CLI link and the app; `~/.config/reap` and the listed
clones stay as they are. To keep it updated from this clone, see the
[repository README](../../README.md#updates).

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
13). `~/.config/reap/state.json` holds the grace clocks, *Keep*s and refused removals; a
*Pause* file lives beside both.

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
