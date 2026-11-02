import type { Meta, StoryObj } from "@storybook/react-vite";
import { userEvent, within } from "storybook/test";

import { SCENARIOS } from "./api.scenarios";
import { openAddForm } from "./popoverSteps";
import { ScenarioPopover } from "./ScenarioPopover";

/** `docs/design/popover/reference/add-form-r4.png`, one story per panel. */
const meta = {
  title: "Popover/Add form",
  component: ScenarioPopover,
  argTypes: { scenario: { control: "select", options: Object.keys(SCENARIOS) } },
  args: { scenario: "regular" },
} satisfies Meta<typeof ScenarioPopover>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Start: Story = {
  play: ({ canvasElement }) => openAddForm(canvasElement, { choose: false }),
};

export const ReadyFollow: Story = {
  name: "Ready, follow",
  play: ({ canvasElement }) => openAddForm(canvasElement),
};

export const Mirror: Story = {
  args: { options: { pickedFolder: "~/Mirrors/example-wiki" } },
  play: async ({ canvasElement }) => {
    await openAddForm(canvasElement);
    const canvas = within(canvasElement);
    await userEvent.click(canvas.getByRole("radio", { name: /mirror/ }));
    await userEvent.type(canvas.getByRole("textbox", { name: "Clone URL" }), "https://example.com/team/wiki.git");
  },
};

export const Commands: Story = {
  args: { options: { pickedFolder: "~/Development/example-handbook" } },
  play: async ({ canvasElement }) => {
    await openAddForm(canvasElement);
    const canvas = within(canvasElement);
    await userEvent.click(canvas.getByRole("button", { name: "Add commands…" }));
    await userEvent.type(canvas.getByRole("textbox", { name: "Gate" }), 'test -z "$(git status --porcelain)"');
    await userEvent.type(canvas.getByRole("textbox", { name: "After" }), "./scripts/reconcile.sh");
  },
};

/** The fake holds the add pending for good. */
export const Adding: Story = {
  args: { scenario: "adding" },
  play: async ({ canvasElement }) => {
    await openAddForm(canvasElement);
    await userEvent.click(within(canvasElement).getByRole("button", { name: "Add" }));
  },
};

/** The picked folder is already listed, so the fake refuses it as the CLI does. */
export const AddError: Story = {
  name: "Error",
  args: { scenario: "add-refused" },
  play: async ({ canvasElement }) => {
    await openAddForm(canvasElement);
    await userEvent.click(within(canvasElement).getByRole("button", { name: "Add" }));
  },
};

export const Light: Story = {
  name: "Light · ready, follow",
  globals: { theme: "light" },
  play: ({ canvasElement }) => openAddForm(canvasElement),
};
