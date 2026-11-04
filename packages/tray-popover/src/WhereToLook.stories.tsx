import type { Meta, StoryObj } from "@storybook/react-vite";
import { fn } from "storybook/test";

import { PopoverFooter } from "./PopoverFooter";
import { WhereToLook, WhereToLookFooter } from "./WhereToLook";

const NOTE = "Desktop, Documents and Downloads are searched only if you add them here. macOS asks you first.";

const meta = {
  title: "Components/WhereToLook",
  component: WhereToLook,
  // The view brings its own gutters: a bare 360px surface, with the footer band below it as the tool draws it.
  decorators: [
    (Story, context) => (
      <div style={{ width: 360, background: "var(--color-surface)", color: "var(--color-text)", fontSize: "var(--text-sm)" }}>
        <Story />
        <PopoverFooter>
          <WhereToLookFooter
            doneLabel="Done"
            onDone={fn()}
            lookAgainLabel="Look again"
            onLookAgain={fn()}
            lookAgainDisabled={context.args.roots.length === 0}
          />
        </PopoverFooter>
      </div>
    ),
  ],
  args: {
    foldersTitle: "Folders",
    roots: [
      { path: "~/Development", found: "5 clones" },
      { path: "~/Code", found: "3 clones" },
    ],
    addFolderLabel: "Add folder…",
    onAddRoot: fn(),
    onRemoveRoot: fn(),
    removeRootLabel: (path) => `Stop looking in ${path}`,
    note: NOTE,
    skippedTitle: "Skipped",
    skipped: [
      { path: "~/Development/scratch", name: "scratch", root: "~/Development" },
      { path: "~/Code/old-site", name: "old-site", root: "~/Code" },
    ],
    unskipLabel: "Unskip",
    onUnskip: fn(),
  },
} satisfies Meta<typeof WhereToLook>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Panel 9: folders and skipped clones. */
export const FoldersAndSkipped: Story = {};

/** Nothing skipped: the Skipped section is hidden. */
export const NothingSkipped: Story = { args: { skipped: [] } };

/** A root the last scan could not read shows its error in place of the count. */
export const UnreadableRoot: Story = {
  args: {
    roots: [
      { path: "~/Development", found: "5 clones" },
      { path: "~/work", error: "not allowed to read it" },
    ],
  },
};

/** A root that has not been scanned yet has no count. */
export const NotScannedYet: Story = {
  args: { roots: [{ path: "~/Development" }, { path: "~/Code" }] },
};

/** The last root removed: only Add folder… is left, and the footer's Look again is disabled. */
export const NoFolders: Story = { args: { roots: [], skipped: [] } };
