# CLAUDE.md

git-tether (`git tether`) keeps the local git clones you list on one branch each, from a macOS
menu-bar app and a CLI. Spec: [ADR 0001](docs/adr/0001-tether-keeps-listed-repos-on-their-branch.md).

Part of steading: the repo-wide commands, commit rules and owner paths are in the
[root CLAUDE.md](../../CLAUDE.md). Paths and commands below are relative to this folder
(`tools/tether/`).

## Commands

- `bash -n install.sh` and `./install.sh --dry-run` — installer syntax and dry run; never run it without `--dry-run` while developing
- `scripts/render-icons.sh` — the app and tray icons from `app/icons/src/*.svg`; commit sources and PNGs together

UI (`app/ui/`, the popover; [ADR 0002](docs/adr/0002-the-tray-ui-is-a-react-popover.md), [styleguide](docs/styleguide.md)):

- `npm ci` at the repository root — installs every popover's pinned dependencies (never `npm install` inside `app/ui`)
- `npm --prefix app/ui test -- --run` — component tests (Vitest, Testing Library, the fake command layer)
- `npm --prefix app/ui run typecheck` — TypeScript, strict
- `npm --prefix app/ui run dev` — the popover in a browser against `src/api.fake.ts`
- `npm --prefix app/ui run build` — the bundle into `app/dist/` (`cargo tauri build` runs it)
- `npm --prefix app/ui run storybook` — every designed popover state as a story, on port 6006, dark and light from the toolbar ([ADR 0005](docs/adr/0005-the-popover-states-live-in-storybook.md))
- `npm --prefix app/ui run build-storybook` — the static Storybook into `app/ui/storybook-static/` (git-ignored)
