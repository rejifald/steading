import { PopoverFooter } from "@steading/tray-popover/PopoverFooter";
import { WhereToLook, WhereToLookFooter } from "@steading/tray-popover/WhereToLook";

import type { Discovery } from "../api";
import { ListScroll } from "../worktree-list/ListScroll";
import { clonesText } from "./copy";

const NOTE = "Desktop, Documents and Downloads are searched only if you add them here. macOS asks you first.";

interface RootsViewProps {
  discovery: Discovery;
  onAddRoot: () => void;
  onRemoveRoot: (path: string) => void;
  onUnskip: (path: string) => void;
  onDone: () => void;
  onLookAgain: () => void;
}

/** The "Where to look" view: the folders a scan walks and the clones skipped, with its own footer. */
export function RootsView({ discovery, onAddRoot, onRemoveRoot, onUnskip, onDone, onLookAgain }: RootsViewProps) {
  return (
    <>
      <ListScroll>
        <WhereToLook
          foldersTitle="Folders"
          roots={discovery.roots.map((root) => ({
            path: root.path,
            ...(root.error === null ? {} : { error: root.error }),
            ...(root.clones === null || root.error !== null ? {} : { found: clonesText(root.clones) }),
          }))}
          addFolderLabel="Add folder…"
          onAddRoot={onAddRoot}
          onRemoveRoot={onRemoveRoot}
          removeRootLabel={(path) => `Stop looking in ${path}`}
          note={NOTE}
          skippedTitle="Skipped"
          skipped={discovery.skipped}
          unskipLabel="Unskip"
          onUnskip={onUnskip}
        />
      </ListScroll>
      <PopoverFooter>
        <WhereToLookFooter
          doneLabel="Done"
          onDone={onDone}
          lookAgainLabel="Look again"
          onLookAgain={onLookAgain}
          lookAgainDisabled={discovery.roots.length === 0}
        />
      </PopoverFooter>
    </>
  );
}
