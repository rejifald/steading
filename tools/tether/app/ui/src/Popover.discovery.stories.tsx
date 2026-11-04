import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, userEvent, within } from "storybook/test";

import { SCENARIOS } from "./api.scenarios";
import { expandFound, findClones, openWhereToLook } from "./popoverSteps";
import { ScenarioPopover } from "./ScenarioPopover";

/**
 * `docs/design/discovery/reference/discovery.dc.html`, one story per tether panel (1 to 9);
 * panels 10 and 11 are reap's.
 */
const meta = {
  title: "Popover/Discovery",
  component: ScenarioPopover,
  argTypes: { scenario: { control: "select", options: Object.keys(SCENARIOS) } },
  args: { scenario: "invite" },
} satisfies Meta<typeof ScenarioPopover>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Panel 1, idle: nothing listed, nothing scanned. */
export const Invite: Story = {};

/**
 * Panel 1, live: *Find clones* starts the scan, Found fills in a root at a time (the fake takes
 * a moment per root), and the header hairline runs. Press *Add* or the ✕ on each row.
 */
export const Live: Story = {
  name: "Live · find clones",
  args: { options: { scanStep: 1500 } },
  play: async ({ canvasElement }) => {
    await findClones(canvasElement);
    await within(canvasElement).findByRole("button", { name: "Skip cache" });
  },
};

/** Panel 1, settled: the last suggestion answered (here the scan found no clone at all). */
export const Settled: Story = { args: { scenario: "no-suggestions" } };

/** Panel 2: partway through a scan, the second root being walked. */
export const Looking: Story = { args: { scenario: "looking" } };

/** Panel 3: a finished scan with nothing listed. */
export const Found: Story = { args: { scenario: "found" } };

/** Panel 4: two added (one syncing), four still found; the skip below keeps the section open. */
export const Triage: Story = {
  args: { scenario: "triage" },
  play: async ({ canvasElement }) => {
    await expandFound(canvasElement);
    await userEvent.click(await within(canvasElement).findByRole("button", { name: "Skip wiki" }));
    await expect(within(canvasElement).getByRole("region", { name: "Found" })).toBeInTheDocument();
  },
};

/** Panel 5: two left unanswered three days ago, *Look again* on the row. */
export const Later: Story = { args: { scenario: "later" } };

/** Panel 5, opened from the chevron. */
export const LaterExpanded: Story = {
  name: "Later · expanded",
  args: { scenario: "later" },
  play: ({ canvasElement }) => expandFound(canvasElement),
};

/** Panel 6: looked again, every clone found is already listed or skipped. */
export const LookedAgain: Story = { name: "Looked again", args: { scenario: "looked-again" } };

/** Panel 7: the first look found nothing; one root stopped at the folder cap. */
export const FoundNothing: Story = { name: "Found nothing", args: { scenario: "found-nothing" } };

/** Panel 8: one folder unreadable, the rest shows. */
export const FoundError: Story = { name: "Error · folder unreadable", args: { scenario: "found-error" } };

/** Panel 9: folders and skipped clones. */
export const WhereToLook: Story = {
  name: "Where to look",
  args: { scenario: "where-to-look" },
  play: ({ canvasElement }) => openWhereToLook(canvasElement),
};

export const WhereToLookNoRoots: Story = {
  name: "Where to look · no folder left",
  args: { scenario: "where-to-look-no-roots" },
  play: async ({ canvasElement }) => {
    await userEvent.click(await within(canvasElement).findByRole("button", { name: "Where to look…" }));
    await within(canvasElement).findByRole("heading", { name: "Where to look", level: 1 });
  },
};

/** The invitation when no scan root exists: it looks nowhere yet. */
export const InviteNoRoots: Story = { name: "Invite · no roots", args: { scenario: "invite-no-roots" } };

/** Four roots read as the first two and a count. */
export const InviteManyRoots: Story = { name: "Invite · many roots", args: { scenario: "invite-many-roots" } };

/** Long names and paths truncate; the full path is the tooltip. */
export const LongNames: Story = { name: "Long names", args: { scenario: "found-long-names" } };

/** *Add* is in flight: the row stays with *Add* disabled, and the hairline runs. */
export const AddInFlight: Story = {
  name: "Add in flight",
  args: { scenario: "add-pending" },
  play: async ({ canvasElement }) => {
    const row = (await within(canvasElement).findByRole("button", { name: "Skip cache" })).closest("li")!;
    await userEvent.click(within(row).getByRole("button", { name: "Add" }));
  },
};

/** *Add* is refused with `add`'s own text, in the notice strip; the row stays. */
export const AddFailed: Story = {
  name: "Add failed",
  args: { scenario: "add-fails" },
  play: async ({ canvasElement }) => {
    const row = (await within(canvasElement).findByRole("button", { name: "Skip search" })).closest("li")!;
    await userEvent.click(within(row).getByRole("button", { name: "Add" }));
    await within(canvasElement).findByRole("alert");
  },
};

export const LightInvite: Story = { name: "Light · invite", globals: { theme: "light" } };

export const LightFound: Story = { name: "Light · found", args: { scenario: "found" }, globals: { theme: "light" } };

export const LightLooking: Story = { name: "Light · looking", args: { scenario: "looking" }, globals: { theme: "light" } };

export const LightLater: Story = { name: "Light · later", args: { scenario: "later" }, globals: { theme: "light" } };

export const LightFoundNothing: Story = {
  name: "Light · found nothing",
  args: { scenario: "found-nothing" },
  globals: { theme: "light" },
};

export const LightWhereToLook: Story = {
  name: "Light · where to look",
  args: { scenario: "where-to-look" },
  globals: { theme: "light" },
  play: ({ canvasElement }) => openWhereToLook(canvasElement),
};
