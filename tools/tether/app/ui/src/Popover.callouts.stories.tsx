import type { Meta, StoryObj } from "@storybook/react-vite";
import { userEvent, within } from "storybook/test";

import { SCENARIOS } from "./api.scenarios";
import { dismissIn, tabTo } from "./popoverSteps";
import { ScenarioPopover } from "./ScenarioPopover";

/** `docs/design/popover/reference/callouts-r6.png`, one story per panel. */
const meta = {
  title: "Popover/Callouts",
  component: ScenarioPopover,
  argTypes: { scenario: { control: "select", options: Object.keys(SCENARIOS) } },
  args: { scenario: "stacked" },
} satisfies Meta<typeof ScenarioPopover>;

export default meta;
type Story = StoryObj<typeof meta>;

const SITE = "~/Development/example-site";
const HANDBOOK = "~/Development/example-handbook";
const CLI = "~/Development/example-cli";

export const ThreeNeedYouStacked: Story = { name: "Three need you · stacked" };

export const ShowAll: Story = {
  name: "Show all",
  play: async ({ canvasElement }) => {
    await userEvent.click(await within(canvasElement).findByRole("button", { name: "Show all" }));
  },
};

export const Dismiss: Story = {
  args: { scenario: "regular" },
  play: async ({ canvasElement }) => {
    const callout = await within(canvasElement).findByRole("region", { name: "example-site needs you" });
    await tabTo(within(callout).getByRole("button", { name: "Dismiss until it changes" }));
  },
};

export const OneDismissed: Story = {
  name: "One dismissed",
  beforeEach: () => dismissIn("stacked", [SITE]),
};

export const AllDismissed: Story = {
  name: "All dismissed",
  beforeEach: () => dismissIn("stacked", [SITE, HANDBOOK, CLI]),
  play: async ({ canvasElement }) => {
    // The panel shows the first dismissed row's tooltip, as hover opens it.
    await userEvent.hover(await within(canvasElement).findByRole("listitem", { name: "example-site" }));
  },
};

/** example-site was dismissed while on another branch; its reason is now `HEAD is detached`. */
export const ItChanged: Story = {
  name: "It changed",
  args: { scenario: "reason-changed" },
  beforeEach: () => dismissIn("stacked", [SITE]),
};

export const Light: Story = { name: "Light · three need you, stacked", globals: { theme: "light" } };
