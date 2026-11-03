# CLAUDE.md

steading builds two macOS menu-bar apps that keep a developer's local git in shape, each also a
`git` subcommand:

- [`tools/tether/`](tools/tether/) — **git-tether** (`git tether`) keeps the clones you list on
  one branch each.
- [`tools/reap/`](tools/reap/) — **git-reap** (`git reap`) removes the worktrees whose work has
  merged.

They are separate products in one repository ([ADR 0001](docs/adr/0001-one-repo-builds-tether-and-reap.md)):
each has its own version, changelog, ADRs and `CLAUDE.md`, which you read before working in it.

Personal imports (your own conventions, anything on your machine) go in `CLAUDE.local.md`, which
git ignores. This file is public and holds only what binds every contributor.

## Commands

From the repository root:

- `cargo fmt --all --check` — formatting
- `cargo clippy --workspace --all-targets --locked -- -D warnings` — lint (clippy pedantic is on
  for the workspace; `#[allow]` a lint only with a reason comment)
- `cargo test --workspace --locked` — tests (default members are the two cores; `--workspace`
  adds the Tauri apps, which build only on macOS)
- `scripts/check-sensitive.sh --all` — the sensitive-info check over every tracked file
- `lefthook install` — once per clone, so the hooks below run (`brew install lefthook`)

The UI is one npm workspace (`packages/tray-popover` and both popovers):

- `npm ci` — install every popover's pinned dependencies; always at the root, never inside `app/ui`
- `npm --prefix packages/tray-popover test -- --run` — the shared package's tests
- `npm --prefix packages/tray-popover run typecheck` — the shared package's types

Each tool's UI, icon and installer commands are in its own `CLAUDE.md`.

## Layout

Code is grouped by tool. Something moves out of `tools/<tool>/` only when both tools use it, and
then into a crate or package named for what it does (never `common` or `shared`).

| Path | What |
|---|---|
| `tools/<tool>/core/` | `<tool>-core`: the logic and the UI-free `git-<tool>` CLI. Builds and tests on Linux. |
| `tools/<tool>/app/` | `<tool>-app`: the Tauri v2 menu-bar shell; the same binary is the CLI. macOS only. |
| `tools/<tool>/app/ui/` | The popover: React, Base UI and Tailwind; a member of the root npm workspace. |
| `packages/tray-popover/` | `@steading/tray-popover`: the controls, icons and popover pieces both popovers use. Source-only. [README](packages/tray-popover/README.md), [ADR 0003](docs/adr/0003-the-popovers-share-one-npm-workspace.md). |
| `tools/<tool>/docs/` | The tool's ADRs, styleguide, brand and design notes. |
| `docs/adr/` | Decisions that cover the whole repository. |
| `scripts/` | Repo gates and the SVG renderer both tools' icon scripts use. |
| `package.json`, `package-lock.json` | The npm workspace and its one lockfile; raise a dependency here, once. |
| `rust-toolchain.toml` | The one Rust version every clone and CI build with; raising it is its own commit. |

## Commits

This repository is public. Everything in a commit is published for good: the diff, the message,
and the author and committer identity. Rewriting history afterwards does not take it back.

- **Messages:** Conventional Commits (`type(scope): summary`, imperative, one logical change per
  commit), in English. The scope is the tool (`tether`, `reap`) or the area (`ci`, `ws`), so a
  tool's changelog can be read from its history. The body says what changed and why.
- **Identity:** commit as your GitHub noreply address (`<id>+<login>@users.noreply.github.com`).
  Set it per clone with `git config user.email`; never a personal or work address.
- **No sensitive information** in a diff, a message, a branch name or a file name:
  - no local paths: no absolute home or temp directory (`/Users/<name>`, `/home/<name>`,
    `/private/tmp`, `/var/folders`) and no path that exists only on your machine. Write `~/…`,
    `$HOME`, or the placeholder `/Users/me` in tests;
  - no secrets: tokens, API keys, passwords, private keys, `.env` contents, credentials in URLs;
  - no personal data: email addresses, phone numbers, names of people who are not contributors;
  - no private infrastructure: hostnames, IPs, runner or machine names, private repository
    names and links into private repositories;
  - no unredacted command output (logs, `git config`, `env` and stack traces carry all of the
    above).
- **Gate:** `lefthook.yml` runs `scripts/check-sensitive.sh` on the staged diff (pre-commit) and
  on the message (commit-msg), and refuses a commit whose author email is not a noreply address.
  A hit means rewriting the line, never skipping the hook (`--no-verify`). A false positive is
  fixed in the script's allowlist, in its own commit.
- If something sensitive was pushed anyway, say so at once. Deleting it in a new commit is not
  enough: the fix is a history rewrite, or recreating the repository.

## Owner paths

`.github/owner-paths` lists the files whose changes need the owner's approval: CI, the hooks,
the sensitive-info gate and the installer.
