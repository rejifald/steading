import type { Meta, StoryObj } from "@storybook/react-vite";
import type { ComponentType, SVGProps } from "react";

import * as icons from "./icons";

const ICONS = icons as Record<string, ComponentType<SVGProps<SVGSVGElement>>>;

/** A row per icon: the glyph, then its export name. */
const GRID = { display: "grid", gridTemplateColumns: "24px 1fr", gap: 8, alignItems: "center" } as const;

interface GalleryProps {
  /** A colour token, as `text-<token>` gives it. */
  color: string;
}

/** Every icon in `icons.tsx`, at its own 16px, in one token colour. */
function Gallery({ color }: GalleryProps) {
  return (
    <div style={{ ...GRID, color: `var(--color-${color})` }}>
      {Object.entries(ICONS).map(([name, Icon]) => [
        <Icon key={`${name}-icon`} />,
        <code key={name} style={{ color: "var(--color-muted)" }}>
          {name}
        </code>,
      ])}
    </div>
  );
}

const meta = {
  title: "Components/Icons",
  component: Gallery,
  parameters: { panel: true },
  argTypes: {
    color: { control: "select", options: ["text", "muted", "faint", "accent", "needs-you"] },
  },
  args: { color: "text" },
} satisfies Meta<typeof Gallery>;

export default meta;
type Story = StoryObj<typeof meta>;

export const All: Story = {};

export const NeedsYouColour: Story = { args: { color: "needs-you" } };
