# @steading/tray-popover

The pieces of the menu-bar popover that tether and reap draw the same way
([ADR 0003](../../docs/adr/0003-the-popovers-share-one-npm-workspace.md)). Source only: each app's
Vite bundles what it imports. There is no barrel; import each file:

```ts
import { Callout } from "@steading/tray-popover/Callout";
import { TextButton } from "@steading/tray-popover/controls/TextButton";
```

and, in the app's `src/theme.css`, after Tailwind:

```css
@import "tailwindcss";
@import "@steading/tray-popover/theme.css";
```

| File | What |
|---|---|
| `src/theme.css` | the shared tokens (surfaces, text, border, focus, needs-you, radii, fonts, type scale, `shadow-raised`, `animate-progress`) and the window's base rules. Each app adds its accent, its state colours and `--color-progress`. |
| `src/controls/TextButton.tsx`, `IconButton.tsx` | labelled and icon-only buttons |
| `src/controls/icons.tsx` | the icon set, named by shape; each tool maps its states to them |
| `src/Callout.tsx` | the raised box with a top strip (tones `needs-you`, `plain`) |
| `src/ProgressLine.tsx` | the 2px hairline while something runs |
| `src/NoticeStrip.tsx` | the tinted strip under the header for a failure, verbatim |
| `src/ReadError.tsx` | the full-panel body when the tool's config cannot be read |
| `src/PopoverFrame.tsx`, `PopoverHeader.tsx`, `PopoverFooter.tsx` | the panel, its top band and its bottom band |

Nothing here knows a tool's states, words or data; that stays in `tools/<tool>/app/ui/`. A piece
moves here when the second tool needs it, and leaves the first in the same change.

Commands, from the repository root (`npm ci` there installs everything):

- `npm --prefix packages/tray-popover test -- --run` — component tests
- `npm --prefix packages/tray-popover run typecheck` — TypeScript, strict

The stories (`*.stories.tsx`) show in tether's Storybook:
`npm --prefix tools/tether/app/ui run storybook`.
