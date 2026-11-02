import type { Meta, StoryObj } from "@storybook/react-vite";

import { Callout } from "./Callout";
import { NeedsYouIcon } from "./controls/icons";
import { TextButton } from "./controls/TextButton";

const meta = {
  title: "Components/Callout",
  component: Callout,
  parameters: { panel: true },
  argTypes: { tone: { control: "inline-radio", options: ["needs-you", "plain"] } },
  args: {
    tone: "needs-you",
    heading: (
      <>
        <NeedsYouIcon className="shrink-0 text-needs-you" />
        <span className="font-medium">Not added</span>
      </>
    ),
    children: <p className="text-needs-you">~/Development/example-app is already listed</p>,
  },
} satisfies Meta<typeof Callout>;

export default meta;
type Story = StoryObj<typeof meta>;

export const NeedsYou: Story = { args: { role: "alert" } };

export const Plain: Story = {
  args: {
    tone: "plain",
    heading: <span>Stop keeping example-api on main?</span>,
    children: (
      <>
        <p className="text-muted">The clone stays as it is.</p>
        <div className="flex gap-2">
          <TextButton variant="inset" onClick={() => {}}>
            Remove
          </TextButton>
          <TextButton variant="inset" onClick={() => {}}>
            Keep
          </TextButton>
        </div>
      </>
    ),
  },
};

export const HeadingOnly: Story = { args: { children: undefined } };
