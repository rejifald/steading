import type { Plugin } from "vite";

/** The one rule in each `theme.css` (the package's and the app's) that switches the tokens to dark. */
const DARK_MEDIA = "@media (prefers-color-scheme: dark) {";

/**
 * Storybook-only: `theme.css` picks dark or light from the system, which a story cannot set.
 * Rewrites the dark block of every `src/theme.css` to follow `<html data-theme="dark">` instead,
 * which the toolbar's Theme switch sets (`decorators.tsx`). No app build runs this plugin.
 */
export function themeByAttribute(): Plugin {
  return {
    name: "tray-popover:theme-by-attribute",
    enforce: "pre",
    transform(code, id) {
      if (!id.endsWith("/src/theme.css")) return null;
      const start = code.indexOf(DARK_MEDIA);
      const root = code.indexOf(":root {", start);
      if (start === -1 || root === -1) {
        throw new Error(`${id} has no "${DARK_MEDIA} :root {" block; update themeByAttribute.ts`);
      }
      // `:root[data-theme="dark"] { & { …tokens } }`: the same tokens, nested under the attribute.
      return (
        code.slice(0, start) +
        ':root[data-theme="dark"] {' +
        code.slice(start + DARK_MEDIA.length, root) +
        "& {" +
        code.slice(root + ":root {".length)
      );
    },
  };
}
