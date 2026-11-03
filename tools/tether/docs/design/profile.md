# Design profile — git-tether

Where this project keeps its design system. The method lives in the design skills; this file
only says where things are.

## Tokens

- The shared tokens (surfaces, text, border, focus, radii, fonts, type scale, shadow) live in
  `packages/tray-popover/src/theme.css`; `app/ui/src/theme.css` imports it and adds only
  tether's own (`--color-accent`, `--color-in-sync`, `--color-behind`, `--color-skipped`,
  `--color-progress`). Dark values sit under `prefers-color-scheme: dark`. Together they are
  the only place a colour, length, radius, shadow or type size is defined. The table of what
  each token means is in [docs/styleguide.md](../docs/styleguide.md#tokens).
- Tailwind's own palette, radii, type scale and fonts are cleared, so only token utilities
  exist: `bg-surface`, `text-muted`, `rounded-control`, `text-sm`, `p-3`.
- Spacing is a 4px grid (`--spacing: 4px`): padding, gaps and sizes are multiples of 4.
- Forbidden: arbitrary values (`bg-[#fff]`, `w-[361px]`), `style=` except for values computed
  at runtime, any CSS library besides Tailwind.

## Component libraries

| Source | Scope |
|---|---|
| `@base-ui/react` | every interactive primitive (Button, Field, Input, RadioGroup/Radio, Tooltip, ScrollArea, Separator); never a hand-rolled clickable `div` |
| `@steading/tray-popover` (`packages/tray-popover/`) | what both popovers use: `controls/TextButton`, `controls/IconButton`, `controls/icons` (named by shape), `Callout`, `ProgressLine`, `NoticeStrip`, `ReadError`, `PopoverHeader`, `PopoverFooter`, `PopoverFrame`; import as `@steading/tray-popover/<File>` |
| `app/ui/src/StateLabel.tsx` | a state as a glyph and the CLI's word (`GLYPHS`) |

There is no barrel; import each file directly.

## Where a new component goes

A component used by both tools goes in `packages/tray-popover/` (with its test and story); one
used by one tool lives in that app. Inside the app, a component used by one feature lives in
that feature's folder (`repo-list/`, `add-form/`); one used by two or more features moves to
`src/`. A new dependency in `app/ui/` is reviewed first (ADR 0002).

## Test harnesses

| Harness | Config | Command | Covers |
|---|---|---|---|
| Vitest + Testing Library + jsdom | `app/ui/vite.config.ts`, `src/test-setup.ts` | `npm --prefix app/ui test -- --run` | components against the fake command layer `src/api.fake.ts`: roles, names, text, interaction |
| Vitest (package) | `packages/tray-popover/vite.config.ts` | `npm --prefix packages/tray-popover test -- --run` | the shared controls and popover pieces on their own |
| TypeScript | `app/ui/tsconfig.json` | `npm --prefix app/ui run typecheck` | strict types |
| Storybook 10 (react-vite) | `app/ui/.storybook/main.ts`, `preview.tsx` | `npm --prefix app/ui run storybook` (port 6006), `run build-storybook` | one story per panel of the approved boards (`Popover/*`, named like the captions), leaf components with controls (`Components/*`); `play` steps in `src/popoverSteps.ts`; the package's own stories (`packages/tray-popover/`) show here too |

No visual-regression or a11y harness yet ([ADR 0005](../docs/adr/0005-the-popover-states-live-in-storybook.md)
§ 5); stories are for looking and comparing.

## Breakpoints and artboard viewports

None: the popover is a fixed 360px wide, at most 480px tall. Artboards are drawn at 360×480,
several states side by side, dark and light.

## Language and content

English, sentence case, short; buttons are verbs. State words and reasons are the CLI's own
(`STATE_LABELS` in `StateLabel.tsx`, the `details` the app sends). Paths show with `~`. Ages use
`repo-list/ago.ts`. Full rules: [styleguide § Copy](../docs/styleguide.md#copy).

## Design docs home

`docs/design/<surface>/anatomy.md`, with frozen artboard images in `docs/design/<surface>/reference/`.
Read `docs/styleguide.md` and ADR 0002 first.

## Capturing the current state

Storybook first: `npm --prefix app/ui run storybook` (or the `tether-storybook` entry in
`.claude/launch.json`, port 6006). Each drawn panel is a story in a 360 × 480 frame; the toolbar's
*Theme* switch sets dark or light whatever the system says. One story alone, for a screenshot:
`http://localhost:6006/iframe.html?id=<story-id>&viewMode=story` (add `&globals=theme:light`).
A new drawn state gets a scenario in `api.scenarios.ts` (or a `play` step) and a story in the
same change.

The dev server still renders one scenario per page: `npm --prefix app/ui run dev` (or the `tether-ui` entry in `.claude/launch.json`, port 5173)
serves the popover against the fake data in a browser; set the viewport to 360 wide. Dark or
light follows the system setting. `/?scenario=<name>` renders one drawn state (the names are the
keys of `SCENARIOS` in `app/ui/src/api.scenarios.ts`: `syncing`, `paused`, `stacked`, …).

## Frozen vs free, per surface

| Surface | Frozen | Free |
|---|---|---|
| Tray popover | tokens, Base UI, the CLI's words, 360px width, header/list/footer frame | layout, hierarchy, density, grouping within the frame |
| Native fallback menu (right click) | native macOS menu | nothing |
