import type { Meta, StoryObj } from "@storybook/react-vite";

import { SCENARIOS } from "./api.scenarios";
import { ScenarioPopover } from "./ScenarioPopover";

/** `docs/design/popover/reference/popover-r3.png`, one story per panel. */
const meta = {
  title: "Popover/List",
  component: ScenarioPopover,
  argTypes: { scenario: { control: "select", options: Object.keys(SCENARIOS) } },
  args: { scenario: "regular" },
} satisfies Meta<typeof ScenarioPopover>;

export default meta;
type Story = StoryObj<typeof meta>;

/** The hovered row of this panel is the *Focus · row* story (Edge cases): same actions and tooltip. */
export const Regular: Story = {};

export const Syncing: Story = { args: { scenario: "syncing" } };

export const AllInSync: Story = { name: "All in sync", args: { scenario: "all-in-sync" } };

export const Paused: Story = { args: { scenario: "paused" } };

export const Loading: Story = { args: { scenario: "loading" } };

export const Empty: Story = { args: { scenario: "empty" } };

export const ReadError: Story = { name: "Error", args: { scenario: "error" } };

export const Light: Story = { globals: { theme: "light" } };
