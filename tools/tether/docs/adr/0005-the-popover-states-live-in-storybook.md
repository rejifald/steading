# ADR 0005: the popover's designed states live in Storybook

- **Status:** Accepted
- **Date:** 2026-11-01

## Context

The tray popover was redesigned in six rounds on a design canvas (2026-10-28 to 2026-11-01);
the approved boards are frozen in [`docs/design/popover/reference/`](../design/popover/reference/)
and specified in [the anatomy](../design/popover/anatomy.md). The canvas is a picture of the
design, not the code: to revisit or revise a state later, the owner wants it rendered by the
real components, one state at a time, in dark and light.

Until now the only way to see a state was the dev server with `/?scenario=<name>`
(`app/ui/src/api.scenarios.ts`): one state per page load, no forms opened, no focus or menus, and
dark or light only by switching the whole system. ADR 0002 asks that every new dependency in
`app/ui/` be reviewed first; the owner asked for Storybook, which is that review.

## Decision

1. **Storybook 10 (`storybook`, `@storybook/react-vite`), as dev dependencies only**, pinned
   exact in `app/ui/package.json` and the lockfile. Nothing it brings is imported by
   `src/main.tsx`, so the app bundle (`npm --prefix app/ui run build`) is byte-for-byte what it
   was. Telemetry is off (`core.disableTelemetry`).
2. **One story per panel of the approved boards**, named like the panel captions and grouped
   `Popover/List`, `Popover/Callouts`, `Popover/Add form`, `Popover/Settings`,
   `Popover/Edge cases`. Each renders `Popover` against the fake API (`src/api.fake.ts`) with a
   scenario from `src/api.scenarios.ts`, in a 360 × 480 frame (`src/ScenarioPopover.tsx`). States
   the data does not reach (a form open, focus, an open menu, a tooltip, a dismissal) get a
   `play` function (`src/popoverSteps.ts`). Leaf components (`StateLabel`, `Callout`,
   `RepoLine`, `ProgressLine`, the icons) get a few stories each, under `Components/`, with
   controls.
3. **Dark and light from a toolbar switch, without changing the app.** `theme.css` follows
   `prefers-color-scheme`, which a page cannot set. A Vite plugin that only Storybook loads
   (`.storybook/main.ts`) rewrites that one media block to `:root[data-theme="dark"]`, and the
   switch sets the attribute. It throws if the block is not found, so a change to `theme.css`
   cannot silently break the switch.
4. **Storybook's files are not Tailwind sources.** `theme.css` excludes the stories,
   `ScenarioPopover.tsx`, `popoverSteps.ts` and `.storybook/` with `@source not`, so a word in a
   story never adds a utility to the app's CSS. Stories use only the classes the app uses.
5. **No visual regression yet.** Stories are for looking, comparing and revising. Baselines
   would be a later decision.

## Considered options

- **Keep the dev server and `/?scenario=`.** No new dependency, but a state behind a click or a
  focus can only be reached by hand, every time, and dark/light needs a system switch.
- **Ladle** (a smaller story runner on Vite). Fewer packages, but no `play` functions or
  interaction testing, and a much smaller ecosystem for the owner to lean on later.
- **A hand-written gallery page in the dev server.** No dependency, but it would grow its own
  routing, controls and theme switch — a small Storybook to maintain.
- **`@storybook/addon-themes`** for the switch. One package more for what a toolbar global and a
  ten-line decorator already do.

## Consequences

- `npm --prefix app/ui run storybook` serves the stories on port 6006 (`tether-storybook` in
  `.claude/launch.json`); `npm --prefix app/ui run build-storybook` builds them into
  `app/ui/storybook-static/`, which git ignores. CI does not build Storybook.
- The lockfile grows by 178 dev-only packages (Storybook, its React and Vite integration, Babel,
  react-docgen, oxc parser and resolver bindings, esbuild platform binaries). The production
  dependency tree is unchanged.
- A new designed state is a scenario (or a `play` step) and a story in the same change. The play
  steps drive the real components through roles and names, so a renamed button breaks its story
  loudly.
- **Russian-origin packages.** None is direct and none is shipped. Two transitive, build-time
  packages come in with Storybook (`react-docgen` → `@babel/core` →
  `@babel/helper-compilation-targets`),
  both by Andrey Sitnik: **`browserslist`** and **`update-browserslist-db`**. They run only
  while Storybook compiles stories and are listed here as known exceptions. Checked from each
  added package's npm metadata (author, repository); not checked against company registers.
  Russian-origin build tools already in the lockfile before this ADR (`postcss`, `nanoid`,
  `picocolors`, `css-tree`, through Vite, Testing Library and jsdom) are not new here and are not listed
  in an ADR yet.
