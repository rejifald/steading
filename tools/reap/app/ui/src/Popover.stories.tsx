import type { Meta, StoryObj } from "@storybook/react-vite";
import { userEvent, within } from "storybook/test";

import { SCENARIOS } from "./api.scenarios";
import { ScenarioPopover } from "./ScenarioPopover";

/** Every state of reap's popover (docs/design/popover/anatomy.md, round 2), one story each. */
const meta = {
  title: "Popover/States",
  component: ScenarioPopover,
  argTypes: { scenario: { control: "select", options: Object.keys(SCENARIOS) } },
  args: { scenario: "regular" },
} satisfies Meta<typeof ScenarioPopover>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Regular: Story = {};

export const RemovalPending: Story = { name: "Removal pending", args: { scenario: "removal-pending" } };

export const Tidy: Story = { args: { scenario: "tidy" } };

export const Refreshing: Story = { args: { scenario: "refreshing" } };

/** Remove now on the first ready row; the fake never finishes, so the row stays removing. */
export const Removing: Story = {
  args: { scenario: "removing" },
  play: async ({ canvasElement }) => {
    const row = await within(canvasElement).findByRole("listitem", { name: "fix/login" });
    await userEvent.hover(row);
    await userEvent.click(within(row).getByRole("button", { name: "Remove now" }));
  },
};

export const ConfirmRemoveAll: Story = {
  name: "Confirm remove all",
  args: { scenario: "removal-pending" },
  play: async ({ canvasElement }) => {
    await userEvent.click(await within(canvasElement).findByRole("button", { name: /^Remove all/ }));
  },
};

export const Paused: Story = { args: { scenario: "paused" } };

export const RepoError: Story = { name: "Repo error", args: { scenario: "repo-error" } };

export const Notice: Story = { args: { scenario: "notice" } };

export const StaleReadError: Story = { name: "Read error, last report", args: { scenario: "stale-read-error" } };

export const Loading: Story = { args: { scenario: "loading" } };

export const Empty: Story = { args: { scenario: "empty" } };

export const ReadError: Story = { name: "Read error", args: { scenario: "error" } };

export const Light: Story = { globals: { theme: "light" } };
