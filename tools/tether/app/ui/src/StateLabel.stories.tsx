import type { Meta, StoryObj } from "@storybook/react-vite";

import { StateLabel } from "./StateLabel";

const meta = {
  title: "Components/StateLabel",
  component: StateLabel,
  parameters: { panel: true },
  argTypes: {
    health: { control: "inline-radio", options: ["in-sync", "behind", "skipped", "needs-you"] },
    tone: { control: "inline-radio", options: ["row", "header"] },
  },
  args: { health: "in-sync", tone: "row" },
} satisfies Meta<typeof StateLabel>;

export default meta;
type Story = StoryObj<typeof meta>;

export const InSync: Story = {};

export const BehindWithCount: Story = { args: { health: "behind", behind: 3 } };

export const NeedsYou: Story = { args: { health: "needs-you" } };

export const Syncing: Story = { args: { health: "behind", busy: true } };

export const HeaderNeedsYouCount: Story = {
  args: { health: "needs-you", tone: "header", label: "needs you (3)" },
};

export const HeaderPaused: Story = { args: { health: "in-sync", tone: "header", paused: true } };
