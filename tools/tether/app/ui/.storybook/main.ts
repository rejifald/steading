import type { StorybookConfig } from "@storybook/react-vite";
import type { Plugin } from "vite";

/** The one rule in `src/theme.css` that switches the tokens to dark. */
const DARK_MEDIA = "@media (prefers-color-scheme: dark) {";

/**
 * Storybook-only: `theme.css` picks dark or light from the system, which a story cannot set.
 * Rewrites its dark block to follow `<html data-theme="dark">` instead, which the toolbar's
 * Theme switch sets (`preview.tsx`). The app's own build never runs this plugin.
 */
function themeByAttribute(): Plugin {
  return {
    name: "tether:theme-by-attribute",
    enforce: "pre",
    transform(code, id) {
      if (!id.endsWith("/src/theme.css")) return null;
      const start = code.indexOf(DARK_MEDIA);
      const root = code.indexOf(":root {", start);
      if (start === -1 || root === -1) {
        throw new Error(`theme.css has no "${DARK_MEDIA} :root {" block; update .storybook/main.ts`);
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

const config: StorybookConfig = {
  stories: ["../src/**/*.stories.tsx"],
  framework: "@storybook/react-vite",
  core: { disableTelemetry: true },
  viteFinal: (vite) => ({ ...vite, plugins: [themeByAttribute(), ...(vite.plugins ?? [])] }),
};

export default config;
