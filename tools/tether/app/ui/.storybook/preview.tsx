import type { Decorator, Preview } from "@storybook/react-vite";
import { useEffect } from "react";

import { DISMISSED_KEY } from "../src/repo-list/useDismissed";
import "../src/theme.css";

type Theme = "dark" | "light";

/** Sets `<html data-theme>`, which `main.ts` makes theme.css follow instead of the system. */
const withTheme: Decorator = (Story, context) => {
  const theme = context.globals["theme"] as Theme;
  useEffect(() => {
    document.documentElement.dataset["theme"] = theme;
  }, [theme]);
  return <Story />;
};

/** The popover's surface, 360 wide, for a leaf component (`parameters: { panel: true }`). */
const PANEL = {
  width: 360,
  boxSizing: "border-box",
  padding: 16,
  background: "var(--color-surface)",
  color: "var(--color-text)",
  fontSize: "var(--text-sm)",
} as const;

const withPanel: Decorator = (Story, context) =>
  context.parameters["panel"] ? (
    <div style={PANEL}>
      <Story />
    </div>
  ) : (
    <Story />
  );

const preview: Preview = {
  decorators: [withPanel, withTheme],
  globalTypes: {
    theme: {
      description: "The popover's colour scheme (the app follows the system's)",
      toolbar: {
        title: "Theme",
        icon: "mirror",
        items: [
          { value: "dark", title: "Dark", icon: "moon" },
          { value: "light", title: "Light", icon: "sun" },
        ],
        dynamicTitle: true,
      },
    },
  },
  initialGlobals: { theme: "dark" satisfies Theme },
  parameters: {
    layout: "centered",
    controls: { expanded: true },
  },
  // Dismissals live in localStorage; every story starts with none unless it sets its own.
  beforeEach: () => {
    localStorage.removeItem(DISMISSED_KEY);
  },
};

export default preview;
