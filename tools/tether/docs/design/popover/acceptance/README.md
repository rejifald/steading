# Popover acceptance, 2026-11-01

Each image puts one reference board (top, from [`../reference/`](../reference/)) over the built
popover (bottom), one capture per reference panel in the same order. Captured from the dev server
at 360×480, dark and light, with `/?scenario=<name>` (`app/ui/src/api.scenarios.ts`); the data is
the fake API's, so names and counts differ from the boards. The hovered-row panels are covered by
the focus-row captures (same actions and tooltip): the capture tool's synthetic hover does not
trigger CSS `:hover`.
