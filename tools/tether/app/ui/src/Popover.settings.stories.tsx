import type { Meta, StoryObj } from "@storybook/react-vite";
import { userEvent, within } from "storybook/test";

import { SCENARIOS } from "./api.scenarios";
import { chooseInterval, openSettings } from "./popoverSteps";
import { ScenarioPopover } from "./ScenarioPopover";

/** `docs/design/popover/reference/settings-r4.png`, one story per panel. */
const meta = {
  title: "Popover/Settings",
  component: ScenarioPopover,
  argTypes: { scenario: { control: "select", options: Object.keys(SCENARIOS) } },
  args: { scenario: "regular" },
  play: ({ canvasElement }) => openSettings(canvasElement, "example-api"),
} satisfies Meta<typeof ScenarioPopover>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Regular: Story = {};

export const Changed: Story = {
  play: async ({ canvasElement }) => {
    await openSettings(canvasElement, "example-api");
    await chooseInterval(canvasElement, "15 min");
  },
};

export const NeedsYouCommandsSet: Story = {
  name: "Needs you, commands set",
  args: { scenario: "settings-needs-you" },
  play: ({ canvasElement }) => openSettings(canvasElement, "example-handbook"),
};

export const ConfirmRemove: Story = {
  name: "Confirm remove",
  play: async ({ canvasElement }) => {
    await openSettings(canvasElement, "example-api");
    await userEvent.click(within(canvasElement).getByRole("button", { name: "Remove from tether…" }));
  },
};

/** The fake holds the save pending for good. */
export const Saving: Story = {
  args: { scenario: "adding" },
  play: async ({ canvasElement }) => {
    await openSettings(canvasElement, "example-api");
    await chooseInterval(canvasElement, "15 min");
    await userEvent.click(within(canvasElement).getByRole("button", { name: "Save" }));
  },
};

/** Another writer removed the repo, so the save fails as `app/src/commands.rs` `edited` does. */
export const SaveError: Story = {
  name: "Error",
  args: { scenario: "refused" },
  play: async ({ canvasElement }) => {
    await openSettings(canvasElement, "example-api");
    await chooseInterval(canvasElement, "15 min");
    await userEvent.click(within(canvasElement).getByRole("button", { name: "Save" }));
  },
};

export const Light: Story = { name: "Light · regular", globals: { theme: "light" } };
