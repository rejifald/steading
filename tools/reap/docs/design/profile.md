# Design profile

Where git-reap keeps its design system, for the design skills (design-directions,
design-handover). One surface: the menu-bar popover in `app/ui/`.

## Tokens

- Shared tokens: `packages/tray-popover/src/theme.css`. `app/ui/src/theme.css` (`@theme`)
  imports it and adds only reap's own (`--color-accent`, `--color-open`, `--color-removable`,
  `--color-held`, `--color-progress`). Documented in `docs/styleguide.md`.
- Tailwind v4, CSS-first. Tailwind's own palette, radii, type scale and fonts are cleared, so
  only tokens have utilities: `bg-surface`, `text-muted`, `rounded-control`, `text-xs`.
- Forbidden: arbitrary values (`bg-[#fff]`, `w-[361px]`), `style=` except for runtime values.
- Spacing is Tailwind's numeric scale on a 4px grid (`--spacing: 4px`, so `p-3` = 12px).

## Components

- Shared with git-tether, from `@steading/tray-popover` (`packages/tray-popover/`): TextButton,
  IconButton, icons (named by shape), Callout, ProgressLine, NoticeStrip, ReadError,
  PopoverHeader, PopoverFooter, PopoverFrame. Import as `@steading/tray-popover/<File>`.
- Reap's own, in `app/ui/src/`, grouped by feature: `worktree-list/`, `add-form/`, and the
  top-level `Header`, `Footer`, `Popover`, `StateLabel`.
- Interactive primitives come from Base UI (`@base-ui/react/*`); never a `div` with a click
  handler.
- A new component used by both tools goes in the package; one used by reap goes beside its only
  caller in `app/ui/src/`.

## Tests

- Vitest + Testing Library on jsdom: `npm --prefix app/ui test -- --run`. Tests sit beside the
  source (`*.test.tsx`). Popover-level behaviour is in `src/Popover.test.tsx` against
  `createFakeApi()` (`src/api.fake.ts`). The shared pieces are tested in the package:
  `npm --prefix packages/tray-popover test -- --run`.
- Storybook 10 (`app/ui/.storybook/`, the theme switch from the package):
  `npm --prefix app/ui run storybook` (port 6007, or the `reap-storybook` entry in
  `.claude/launch.json`), `run build-storybook`. One story per state in `src/Popover.stories.tsx`,
  each a key of `SCENARIOS` in `src/api.scenarios.ts`; `src/api.scenarios.test.tsx` renders
  every scenario. One alone: `http://localhost:6007/iframe.html?id=popover-states--<name>&viewMode=story`
  (add `&globals=theme:light`).
- No visual-regression or a11y harness yet.

## Viewport

- One viewport: the popover panel, 360px wide, at most 480px tall; light and dark via
  `prefers-color-scheme`.

## Language

- English UI, sentence case; buttons are verbs. State words and reasons are the CLI's own.

## Design docs

- `docs/design/<surface>/anatomy.md` with frozen images in `docs/design/<surface>/reference/`.
- A surface both tools draw lives at the repository root instead:
  [`docs/design/discovery/`](../../../../docs/design/discovery/anatomy.md) (clone discovery). Its
  reference is the design canvas's `.dc.html` source.
- Reference images and committed screenshots use made-up repository names (the repository
  rules forbid private repository names).

## Capturing the real surface

- `npm --prefix app/ui run dev` serves the popover against the fake API in a plain browser;
  `/?scenario=<name>` renders one of `SCENARIOS`.
  Feed real data by passing `repos` from `target/debug/git-reap status --json` into
  `createFakeApi({ repos })`. Headless Chrome screenshots at `--window-size=360,480`.
