# steading

Two small macOS menu-bar apps that keep your local git in shape. Each is also a `git` subcommand.

| Tool | What it does |
|---|---|
| [**git-tether**](tools/tether/) | Keeps the clones you list on one branch each: fast-forwards them, or mirrors them shallowly, on a timer. |
| [**git-reap**](tools/reap/) | Removes the worktrees whose work has merged, after a grace period, and shows what the rest are waiting on. |

They install and run independently; use either or both.

## Install

Signed releases through Homebrew are on the way. Until then, build from source: you need a Rust
toolchain, git, and Node.js with npm. Each tool's README has its install steps; both run
[`scripts/install-app.sh`](scripts/install-app.sh), which builds the app, copies it to
`~/Applications`, links the `git` subcommand and registers a login item, so the app starts when
you log in.

## Updates

Let git-tether keep your steading clone on `main` and rebuild the apps whenever it moves:

```bash
git tether add ~/Development/steading --branch main --after scripts/reinstall.sh
```

[`scripts/reinstall.sh`](scripts/reinstall.sh) starts in the background and returns at once. It
waits for the clone's `tether.lock`, so a sync cannot move the tree mid-build, then runs
`install.sh --yes` for each tool already in `~/Applications`, reap before tether. It runs outside
tether's process group, because reinstalling tether stops tether. Its output goes to
`~/Library/Logs/steading/reinstall.log`, and a failure posts a notification. It takes `cargo` and
`npm` from your interactive login shell's `PATH`, since launchd's has neither. Run it by hand with
`scripts/reinstall.sh --foreground`.

## Develop

```bash
cargo test --workspace --locked
```

```bash
cargo clippy --workspace --all-targets --locked -- -D warnings
```

The Tauri apps build only on macOS; the cores and CLIs also build and test on Linux. Each
popover is its own npm project under `tools/<tool>/app/ui/`. Commands, layout and commit rules:
[CLAUDE.md](CLAUDE.md). Why one repository: [ADR 0001](docs/adr/0001-one-repo-builds-tether-and-reap.md).

## License

[MIT](LICENSE)
