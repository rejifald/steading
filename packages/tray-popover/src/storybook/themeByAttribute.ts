import type { Plugin } from "vite";

/** Every rule that switches the tokens to dark: the app's own and the one the package's `theme.css` brings in. */
const DARK_BLOCK = /@media\s*\(prefers-color-scheme:\s*dark\)\s*\{(\s*):root\s*\{/g;

/**
 * Storybook-only: `theme.css` picks dark or light from the system, which a story cannot set.
 * Rewrites every dark block to follow `<html data-theme="dark">` instead, which the toolbar's
 * Theme switch sets (`decorators.tsx`). As a normal plugin it runs after Tailwind's `pre` transforms
 * have inlined the package's `theme.css`, so the package's tokens switch too, and before Vite turns
 * the CSS into JS (`enforce: "post"` would see only that JS, or an empty string in a build).
 * No app build runs this plugin.
 */
export function themeByAttribute(): Plugin {
  return {
    name: "tray-popover:theme-by-attribute",
    transform(code, id) {
      if (!id.split("?")[0].endsWith("/src/theme.css")) return null;
      // `:root[data-theme="dark"] { & { …tokens } }`: the same tokens, nested under the attribute.
      const rewritten = code.replace(DARK_BLOCK, (_match, space: string) => `:root[data-theme="dark"] {${space}& {`);
      if (rewritten === code) {
        throw new Error(`${id} has no "@media (prefers-color-scheme: dark) { :root {" block; update themeByAttribute.ts`);
      }
      return rewritten;
    },
  };
}
