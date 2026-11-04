import type { StorybookConfig } from "@storybook/react-vite";
import type { Plugin } from "vite";

/** Every rule that switches the tokens to dark: tether's own and the one the shared package's `theme.css` brings in. */
const DARK_BLOCK = /@media\s*\(prefers-color-scheme:\s*dark\)\s*\{(\s*):root\s*\{/g;

/**
 * Storybook-only: `theme.css` picks dark or light from the system, which a story cannot set.
 * Rewrites every dark block to follow `<html data-theme="dark">` instead, which the toolbar's
 * Theme switch sets (`preview.tsx`). As a normal plugin it runs after Tailwind's `pre` transforms
 * have inlined the package's `theme.css`, so the package's tokens switch too, and before Vite turns
 * the CSS into JS (`enforce: "post"` would see only that JS, or an empty string in a build).
 * The app's own build never runs this plugin.
 */
function themeByAttribute(): Plugin {
  return {
    name: "tether:theme-by-attribute",
    transform(code, id) {
      if (!id.split("?")[0].endsWith("/src/theme.css")) return null;
      // `:root[data-theme="dark"] { & { …tokens } }`: the same tokens, nested under the attribute.
      const rewritten = code.replace(DARK_BLOCK, (_match, space: string) => `:root[data-theme="dark"] {${space}& {`);
      if (rewritten === code) {
        throw new Error(`theme.css has no "@media (prefers-color-scheme: dark) { :root {" block; update .storybook/main.ts`);
      }
      return rewritten;
    },
  };
}

const config: StorybookConfig = {
  // The shared controls, callout and progress line keep their stories beside them in the package.
  stories: ["../src/**/*.stories.tsx", "../../../../../packages/tray-popover/src/**/*.stories.tsx"],
  framework: "@storybook/react-vite",
  core: { disableTelemetry: true },
  viteFinal: (vite) => ({ ...vite, plugins: [themeByAttribute(), ...(vite.plugins ?? [])] }),
};

export default config;
