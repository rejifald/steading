# steading

Two small macOS menu-bar apps that keep your local git in shape. Each is also a `git` subcommand.

| Tool | What it does |
|---|---|
| [**git-tether**](tools/tether/) | Keeps the clones you list on one branch each: fast-forwards them, or mirrors them shallowly, on a timer. |
| [**git-reap**](tools/reap/) | Removes the worktrees whose work has merged, after a grace period, and shows what the rest are waiting on. |

They install and run independently; use either or both.

## Install

Signed releases through Homebrew are on the way. Until then, build from source: you need a Rust
toolchain, git, and Node.js with npm. Each tool's README has its install steps.

## Develop

```bash
cargo test --workspace --locked
```

```bash
cargo clippy --workspace --all-targets --locked -- -D warnings
```

The Tauri apps build only on macOS; the cores and CLIs also build and test on Linux. The
popovers are one npm workspace (`npm ci` at the root) of `tools/<tool>/app/ui/` and the shared
[`packages/tray-popover/`](packages/tray-popover/README.md). Commands, layout and commit rules:
[CLAUDE.md](CLAUDE.md). Why one repository: [ADR 0001](docs/adr/0001-one-repo-builds-tether-and-reap.md).

## License

[MIT](LICENSE)
