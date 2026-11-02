import type { Meta, StoryObj } from "@storybook/react-vite";

import { RepoLine } from "./RepoLine";

const meta = {
  title: "Components/RepoLine",
  component: RepoLine,
  parameters: { panel: true },
  argTypes: {
    health: { control: "inline-radio", options: ["in-sync", "behind", "skipped", "needs-you"] },
    mode: { control: "inline-radio", options: ["follow", "mirror"] },
  },
  args: { name: "example-app", branch: "main", mode: "follow", health: "in-sync" },
} satisfies Meta<typeof RepoLine>;

export default meta;
type Story = StoryObj<typeof meta>;

export const InSync: Story = {};

export const Behind: Story = { args: { name: "example-api", health: "behind", behind: 3 } };

export const SkippedMirror: Story = { args: { name: "example-docs", health: "skipped", mode: "mirror" } };

export const NeedsYou: Story = { args: { name: "example-site", branch: "release", health: "needs-you" } };

export const Syncing: Story = { args: { name: "example-api", health: "behind", busy: true } };

export const Paused: Story = { args: { paused: true } };

export const Preview: Story = { args: { name: "example-new", preview: true } };

export const LongNameTruncates: Story = {
  args: {
    name: "example-monorepo-with-a-really-long-name",
    branch: "feature/2026-q4-refactor-of-everything",
    title: "~/Development/example-monorepo-with-a-really-long-name",
  },
};
