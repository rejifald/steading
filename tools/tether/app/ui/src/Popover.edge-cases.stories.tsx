import type { Meta, StoryObj } from "@storybook/react-vite";
import { userEvent, within } from "storybook/test";

import { minutesAgo, row } from "./api.fake";
import { SCENARIOS } from "./api.scenarios";
import { openAddForm, openIntervalMenu, openSettings, tabTo } from "./popoverSteps";
import { ScenarioPopover } from "./ScenarioPopover";

/**
 * `docs/design/popover/reference/edges-list-r5.png` then `edges-forms-r5.png`, one story per
 * panel.
 */
const meta = {
  title: "Popover/Edge cases",
  component: ScenarioPopover,
  argTypes: { scenario: { control: "select", options: Object.keys(SCENARIOS) } },
  args: { scenario: "regular" },
} satisfies Meta<typeof ScenarioPopover>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Tab lands on the × first (a confirmed divergence, anatomy § 2), then on *Open in Finder*. */
const focusCallout: Story["play"] = async ({ canvasElement }) => {
  const callout = await within(canvasElement).findByRole("region", { name: "example-site needs you" });
  await tabTo(within(callout).getByRole("button", { name: "Open in Finder" }));
};

const focusRow: Story["play"] = async ({ canvasElement }) => {
  const row = await within(canvasElement).findByRole("listitem", { name: "example-api" });
  await tabTo(within(row).getByRole("button", { name: "Sync now" }));
};

export const FocusCallout: Story = { name: "Focus · callout", play: focusCallout };

export const FocusRow: Story = { name: "Focus · row", play: focusRow };

export const LongNames: Story = { name: "Long names", args: { scenario: "long-names" } };

export const Scrolled: Story = {
  args: { scenario: "scrolled" },
  play: async ({ canvasElement }) => {
    await within(canvasElement).findByRole("listitem", { name: "example-data" });
    const viewport = [...canvasElement.querySelectorAll<HTMLElement>("*")].find(
      (element) => element.scrollHeight > element.clientHeight && getComputedStyle(element).overflowY !== "visible",
    );
    viewport?.scrollTo({ top: viewport.scrollHeight / 3 });
  },
};

export const TwoNeedYouSyncing: Story = {
  name: "Two need you, syncing",
  args: { scenario: "two-need-you-syncing" },
};

export const ActionFailed: Story = { name: "Action failed", args: { scenario: "action-failed" } };

export const PausedOneNeedsYou: Story = {
  name: "Paused, one needs you",
  args: { scenario: "paused-needs-you" },
};

export const ReadErrorStaleReport: Story = {
  name: "Read error, stale report",
  args: { scenario: "stale-error" },
};

export const Light: Story = { name: "Light · focus row", globals: { theme: "light" }, play: focusRow };

export const AddLongPath: Story = {
  name: "Add · long path",
  args: { scenario: "add-long-path" },
  play: ({ canvasElement }) => openAddForm(canvasElement),
};

export const AddBranchNotRead: Story = {
  name: "Add · branch not read",
  args: { scenario: "add-branch-not-read" },
  play: async ({ canvasElement }) => {
    await openAddForm(canvasElement);
    await userEvent.click(within(canvasElement).getByRole("textbox", { name: "Branch" }));
  },
};

/** Arrow keys move the mode, which puts the keyboard focus ring on the radio. */
export const AddKeyboard: Story = {
  name: "Add · keyboard",
  play: async ({ canvasElement }) => {
    await openAddForm(canvasElement);
    await tabTo(within(canvasElement).getByRole("radio", { name: /follow/ }));
  },
};

/** The board's repo is in sync on a custom interval, which no scenario has. */
const customInterval = row({
  name: "example-handbook",
  path: "~/Development/example-handbook",
  interval_seconds: 90,
  last_sync: minutesAgo(2, new Date()),
});

export const SettingsMenuOpen: Story = {
  name: "Settings · menu open",
  args: {
    options: {
      rows: [customInterval, ...(SCENARIOS["regular"]?.rows ?? []).filter((r) => r.name !== customInterval.name)],
    },
  },
  play: async ({ canvasElement }) => {
    await openSettings(canvasElement, "example-handbook");
    await openIntervalMenu(canvasElement);
  },
};

export const SettingsLongNameNeverSynced: Story = {
  name: "Settings · long name, never synced",
  args: { scenario: "long-settings" },
  play: ({ canvasElement }) => openSettings(canvasElement, "example-campaign-2026-landing-page"),
};

/** The fake holds the removal pending for good. */
export const SettingsRemoving: Story = {
  name: "Settings · removing",
  args: { scenario: "adding" },
  play: async ({ canvasElement }) => {
    await openSettings(canvasElement, "example-api");
    const canvas = within(canvasElement);
    await userEvent.click(canvas.getByRole("button", { name: "Remove from tether…" }));
    await userEvent.click(canvas.getByRole("button", { name: "Remove" }));
  },
};

