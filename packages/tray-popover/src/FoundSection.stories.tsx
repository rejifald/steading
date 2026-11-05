import type { Meta, StoryObj } from "@storybook/react-vite";
import { fn } from "storybook/test";

import { TextButton } from "./controls/TextButton";
import { FoundPreview, FoundRow, FoundSection } from "./FoundSection";

const actions = { onAdd: fn(), onSkip: fn(), onLookAgain: fn() };

interface Clone {
  name: string;
  detail: string;
}

const TETHER: Clone[] = [
  { name: "api", detail: "main · ~/Development" },
  { name: "web", detail: "main · ~/Development" },
  { name: "infra", detail: "main · ~/Development" },
  { name: "notes", detail: "main · ~/Code" },
  { name: "dotfiles", detail: "master · ~/Code" },
  { name: "blog", detail: "trunk · ~/Code" },
];

const REAP: Clone[] = [
  { name: "api", detail: "7 worktrees · ~/Development" },
  { name: "web", detail: "4 worktrees · ~/Development" },
  { name: "infra", detail: "2 worktrees · ~/Development" },
  { name: "notes", detail: "1 worktree · ~/Code" },
];

function rows(clones: Clone[], busy?: string) {
  return clones.map((clone) => (
    <FoundRow
      key={clone.name}
      name={clone.name}
      detail={clone.detail}
      title={`~/${clone.detail.split("~/")[1]}/${clone.name}`}
      onAdd={actions.onAdd}
      addLabel="Add"
      onSkip={actions.onSkip}
      skipLabel={`Skip ${clone.name}`}
      busy={clone.name === busy}
    />
  ));
}

const meta = {
  title: "Components/FoundSection",
  component: FoundSection,
  // The section brings its own 16px gutters, so it sits on a bare 360px surface, not the padded panel.
  decorators: [
    (Story) => (
      <div style={{ width: 360, background: "var(--color-surface)", color: "var(--color-text)", fontSize: "var(--text-sm)" }}>
        <Story />
      </div>
    ),
  ],
  argTypes: { mode: { control: "inline-radio", options: ["open", "collapsed", "settled"] } },
  args: {
    mode: "open",
    title: "Found",
    count: 6,
    lead: "In ~/Development and ~/Code, just now. Add the ones to keep on their branch.",
    aside: (
      <TextButton size="compact" variant="quiet">
        Where to look…
      </TextButton>
    ),
    children: rows(TETHER),
    onLookAgain: actions.onLookAgain,
    lookAgainLabel: "Look again",
  },
} satisfies Meta<typeof FoundSection>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Panel 3: triage each row. */
export const Open: Story = {};

/** Panel 2 and 1 looking: partial rows, the status at the right, placeholders after them. */
export const Scanning: Story = {
  args: {
    count: 2,
    lead: "You can add these now; more appear as the search goes.",
    aside: undefined,
    scanning: "looking in ~/Code…",
    ghostRows: 3,
    children: rows(TETHER.slice(0, 2)),
  },
};

/** Panel 3, no rows yet: the scan has just started. */
export const ScanningNothingYet: Story = {
  args: { count: 0, aside: undefined, scanning: "looking in ~/Development…", ghostRows: 3, children: undefined },
};

/** Panel 8: one folder unreadable, the rest shows. */
export const ErrorLine: Story = {
  args: {
    count: 2,
    lead: undefined,
    errors: ["Cannot read ~/work: permission denied. ~/Development and ~/Code were searched."],
    children: rows(TETHER.slice(0, 2)),
  },
};

/** Panel 4: listed rows sit above, so the rule separates them. */
export const BelowListedRows: Story = {
  args: { count: 4, lead: undefined, hasRuleAbove: true, children: rows(TETHER.slice(2)) },
};

/** Panel 5: two left unanswered. Click the line to open it. */
export const Collapsed: Story = {
  args: { mode: "collapsed", count: 2, collapsedSummary: "· looked 3 days ago", children: rows(TETHER.slice(0, 2)) },
};

/** Panel 6, and 1 settled: nothing is left. */
export const Settled: Story = {
  args: { mode: "settled", count: 0, settledText: "Nothing new in ~/Development, ~/Code · looked just now", children: undefined },
};

export const SettledNoSuggestions: Story = {
  args: { mode: "settled", count: 0, settledText: "No suggestions · looked just now", children: undefined },
};

/** An Add in flight: its button is disabled and the row stays. */
export const AddInFlight: Story = {
  args: { children: rows(TETHER.slice(0, 3), "web") },
};

/** A long name and path truncate; the title carries the full path. */
export const LongNames: Story = {
  args: {
    count: 2,
    children: rows([
      { name: "a-very-long-clone-folder-name-that-will-not-fit", detail: "main · ~/Development/deeply/nested/place" },
      { name: "short", detail: "release/2026-11-long-branch-name-that-will-not-fit · ~/Code" },
    ]),
  },
};

/** Panel 11: reap's detail is a worktree count. */
export const WorktreeCounts: Story = {
  args: {
    count: 4,
    lead: "Clones with worktrees in ~/Development and ~/Code, just now. Merged worktrees go 24 h after reap first sees them.",
    children: rows(REAP),
  },
};

/** Panels 1 idle and 10: the empty state's dashed box of examples. Not interactive. */
export const Preview: Story = {
  args: {},
  render: () => (
    <div style={{ padding: 24 }}>
      <FoundPreview
        addLabel="Add"
        rows={[
          { name: "your-app", detail: "main · ~/Development" },
          { name: "your-docs", detail: "main · ~/Code" },
        ]}
      />
    </div>
  ),
};
