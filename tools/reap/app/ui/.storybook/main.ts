import type { StorybookConfig } from "@storybook/react-vite";
import { themeByAttribute } from "@steading/tray-popover/storybook/themeByAttribute";

const config: StorybookConfig = {
  // The shared pieces' stories live in the package and show in tether's Storybook.
  stories: ["../src/**/*.stories.tsx"],
  framework: "@storybook/react-vite",
  core: { disableTelemetry: true },
  viteFinal: (vite) => ({ ...vite, plugins: [themeByAttribute(), ...(vite.plugins ?? [])] }),
};

export default config;
