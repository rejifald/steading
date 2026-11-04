import type { Meta, StoryObj } from "@storybook/react-vite";
import { userEvent, within } from "storybook/test";

import { SCENARIOS } from "./api.scenarios";
import { ScenarioPopover } from "./ScenarioPopover";

/**
 * Clone discovery in reap's popover (docs/design/discovery/anatomy.md): the invitation, Found
 * filling in per root, the triage, the quiet rows under the list, and Where to look. Panels 10
 * and 11 of the artboard are reap's; the rest are the shared structure in reap's words.
 */
const meta = {
  title: "Popover/Discovery",
  component: ScenarioPopover,
  argTypes: { scenario: { control: "select", options: Object.keys(SCENARIOS) } },
  args: { scenario: "invite" },
} satisfies Meta<typeof ScenarioPopover>;

export default meta;
type Story = StoryObj<typeof meta>;

type Play = NonNullable<Story["play"]>;

/** Clicks the named buttons in order, each as soon as it appears. */
const press =
  (...names: (string | RegExp)[]): Play =>
  async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    for (const name of names) await userEvent.click(await canvas.findByRole("button", { name }));
  };

/** Clicks *Add* on the row of the named clone. */
const addRow =
  (clone: string): Play =>
  async ({ canvasElement }) => {
    const row = (await within(canvasElement).findByText(clone)).closest("li");
    if (row) await userEvent.click(within(row).getByRole("button", { name: "Add" }));
  };

// Panel 10: nothing listed, no scan yet.
export const Invite: Story = {};

export const InviteLight: Story = { name: "Invite · light", globals: { theme: "light" } };

export const InviteOneRoot: Story = { name: "Invite · one root", args: { scenario: "invite-one-root" } };

export const InviteThreeRoots: Story = { name: "Invite · three or more roots", args: { scenario: "invite-three-roots" } };

export const InviteNoRoots: Story = { name: "Invite · no root", args: { scenario: "invite-no-roots" } };

// Panel 1: Find clones, then the section fills in root by root.
export const FindClones: Story = { name: "Find clones, live", play: press("Find clones") };

// Panel 2: partway through a scan.
export const Looking: Story = { args: { scenario: "found-looking" } };

export const LookingNothingYet: Story = { name: "Looking · nothing yet", args: { scenario: "found-looking-nothing-yet" } };

// Panels 11 and 3: the scan is done; worktree counts, most first.
export const Found: Story = { args: { scenario: "found" } };

export const FoundLight: Story = { name: "Found · light", args: { scenario: "found" }, globals: { theme: "light" } };

export const FoundOne: Story = { name: "Found · one clone", args: { scenario: "found-one" } };

// Panel 8: one folder unreadable, the rest shows.
export const RootError: Story = { name: "Root error", args: { scenario: "found-root-error" } };

// Panel 7: nothing found, nothing listed.
export const FoundNothing: Story = { name: "Found nothing", args: { scenario: "found-nothing" } };

export const FoundNothingStoppedEarly: Story = {
  name: "Found nothing · stopped early",
  args: { scenario: "found-nothing-stopped-early" },
};

export const FoundNothingLight: Story = {
  name: "Found nothing · light",
  args: { scenario: "found-nothing" },
  globals: { theme: "light" },
};

// Panel 4: one added, the rest still found under the list.
export const TriageOneAdded: Story = { name: "Triage · one added", args: { scenario: "found" }, play: addRow("api") };

export const TriageOneSkipped: Story = {
  name: "Triage · one skipped",
  args: { scenario: "found" },
  play: press("Skip tools"),
};

// Panel 4 again, while the scan continues under the list.
export const ListedLooking: Story = { name: "Listed · looking", args: { scenario: "found-listed-scanning" } };

// Panel 5: back later, two left unanswered: one collapsed line under the list.
export const ListedCollapsed: Story = { name: "Listed · collapsed", args: { scenario: "found-listed" } };

export const ListedCollapsedLight: Story = {
  name: "Listed · collapsed · light",
  args: { scenario: "found-listed" },
  globals: { theme: "light" },
};

export const ListedExpanded: Story = {
  name: "Listed · expanded",
  args: { scenario: "found-listed" },
  play: press(/^Found/),
};

export const ListedRootError: Story = { name: "Listed · root error", args: { scenario: "found-listed-error" } };

// Panel 6: looked again, nothing new.
export const ListedNothingNew: Story = { name: "Listed · nothing new", args: { scenario: "found-listed-nothing-new" } };

export const ListedNoSuggestions: Story = {
  name: "Listed · no suggestions",
  args: { scenario: "found-listed-no-suggestions" },
};

// A failed Add: add's own text in the notice, the row stays. And one in flight.
export const AddFails: Story = { name: "Add fails", args: { scenario: "found-add-fails" }, play: addRow("api") };

export const Adding: Story = { args: { scenario: "found-adding" }, play: addRow("api") };

// Panel 9: where to look.
export const WhereToLook: Story = { name: "Where to look", args: { scenario: "found" }, play: press("Where to look…") };

export const WhereToLookLight: Story = {
  name: "Where to look · light",
  args: { scenario: "found" },
  globals: { theme: "light" },
  play: press("Where to look…"),
};

export const WhereToLookNoSkipped: Story = {
  name: "Where to look · nothing skipped",
  args: { scenario: "where-to-look-no-skipped" },
  play: press("Where to look…"),
};

export const WhereToLookRootError: Story = {
  name: "Where to look · root error",
  args: { scenario: "where-to-look-error" },
  play: press("Where to look…"),
};

export const WhereToLookNotScanned: Story = {
  name: "Where to look · not scanned yet",
  args: { scenario: "where-to-look-not-scanned" },
  play: press("Where to look…"),
};

export const WhereToLookNoFolder: Story = {
  name: "Where to look · last folder removed",
  args: { scenario: "found" },
  play: press("Where to look…", "Stop looking in ~/Development", "Stop looking in ~/Code"),
};
