import type { Meta, StoryObj } from "@storybook/react-vite";

import { ProgressLine } from "./ProgressLine";

/** The hairline sits on its positioned parent's bottom edge, here a 32px row. */
const ROW = { position: "relative", height: 32 } as const;

const meta = {
  title: "Components/ProgressLine",
  component: ProgressLine,
  parameters: { panel: true },
  decorators: [
    (Story) => (
      <div style={ROW}>
        <Story />
      </div>
    ),
  ],
  args: { inset: false },
} satisfies Meta<typeof ProgressLine>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Header: Story = {};

export const Row: Story = { args: { inset: true } };
