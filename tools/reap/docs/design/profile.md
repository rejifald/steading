# Design profile

Where git-reap keeps its design system, for the design skills (design-directions,
design-handover). One surface: the menu-bar popover in `app/ui/`.

## Tokens

- Source: `app/ui/src/theme.css` (`@theme`), documented in `docs/styleguide.md`.
- Tailwind v4, CSS-first. Tailwind's own palette, radii, type scale and fonts are cleared, so
  only tokens have utilities: `bg-surface`, `text-muted`, `rounded-control`, `text-xs`.
- Forbidden: arbitrary values (`bg-[#fff]`, `w-[361px]`), `style=` except for runtime values.
- Spacing is Tailwind's numeric scale on a 4px grid (`--spacing: 4px`, so `p-3` = 12px).

## Components

- No shared package. Everything lives in `app/ui/src/`, grouped by feature:
  `controls/` (TextButton, IconButton, icons), `worktree-list/`, `add-form/`, and the
  top-level `Header`, `Footer`, `StateLabel`, `ReadError`.
- Interactive primitives come from Base UI (`@base-ui/react/*`); never a `div` with a click
  handler.
- A new component goes beside its only caller; registering one costs nothing beyond the import.

## Tests

- Vitest + Testing Library on jsdom: `npm --prefix app/ui test -- --run`. Tests sit beside the
  source (`*.test.tsx`). Popover-level behaviour is in `src/Popover.test.tsx` against
  `createFakeApi()` (`src/api.fake.ts`).
- No visual-regression or a11y harness yet.

## Viewport

- One viewport: the popover panel, 360px wide, at most 480px tall; light and dark via
  `prefers-color-scheme`.

## Language

- English UI, sentence case; buttons are verbs. State words and reasons are the CLI's own.

## Design docs

- `docs/design/<surface>/anatomy.md` with frozen images in `docs/design/<surface>/reference/`.
- Reference images and committed screenshots use made-up repository names (the repository
  rules forbid private repository names).

## Capturing the real surface

- `npm --prefix app/ui run dev` serves the popover against the fake API in a plain browser.
  Feed real data by passing `repos` from `target/debug/git-reap status --json` into
  `createFakeApi({ repos })`. Headless Chrome screenshots at `--window-size=360,480`.
