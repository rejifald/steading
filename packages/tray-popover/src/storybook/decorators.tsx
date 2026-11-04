import type { Decorator, Preview } from "@storybook/react-vite";
import { type ReactNode, useEffect } from "react";

type Theme = "dark" | "light";

/** Sets `<html data-theme>`, which `themeByAttribute` makes theme.css follow instead of the system. */
export const withTheme: Decorator = (Story, context) => {
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

export const withPanel: Decorator = (Story, context) =>
  context.parameters["panel"] ? (
    <div style={PANEL}>
      <Story />
    </div>
  ) : (
    <Story />
  );

/** The tray window's size: 360 wide, at most 480 tall. */
const WINDOW = { width: 360, height: 480 } as const;

/** A whole popover in a window-sized frame, for a story of one drawn state. */
export function PopoverWindow({ children }: { children: ReactNode }) {
  return <div style={WINDOW}>{children}</div>;
}

/** What both tools' `preview.tsx` share: the decorators, the Theme toolbar, dark first. */
export const SHARED_PREVIEW = {
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
} satisfies Preview;
