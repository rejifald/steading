import type { StorybookConfig } from "@storybook/react-vite";
import { themeByAttribute } from "@steading/tray-popover/storybook/themeByAttribute";

const config: StorybookConfig = {
  // The shared controls, callout and progress line keep their stories beside them in the package.
  stories: ["../src/**/*.stories.tsx", "../../../../../packages/tray-popover/src/**/*.stories.tsx"],
  framework: "@storybook/react-vite",
  core: { disableTelemetry: true },
  viteFinal: (vite) => ({ ...vite, plugins: [themeByAttribute(), ...(vite.plugins ?? [])] }),
};

export default config;
