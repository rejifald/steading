import type { KeyboardEvent } from "react";
import { useState } from "react";

import { Callout } from "@steading/tray-popover/Callout";
import { TextButton } from "@steading/tray-popover/controls/TextButton";

import type { WorktreeAction } from "../api";

interface RemoveAllButtonProps {
  count: number;
  onClick: () => void;
}

/** "Remove all N" in the Ready heading; it opens the confirm, it removes nothing itself. */
export function RemoveAllButton({ count, onClick }: RemoveAllButtonProps) {
  return (
    <TextButton variant="primary" size="compact" onClick={onClick}>
      {`Remove all ${count}`}
    </TextButton>
  );
}

interface RemoveAllConfirmProps {
  /** The paths of every worktree that is ready to remove. */
  paths: string[];
  /** Runs one action; resolves to whether it succeeded, so removals go one at a time. */
  onAction: (action: WorktreeAction, path: string) => Promise<boolean> | void;
  onClose: () => void;
}

/**
 * The confirm under the Ready heading, in place of its lead line: a plain raised callout, since
 * the heading has no room for it at 360px. Removal is not a state, so it carries no tint.
 */
export function RemoveAllConfirm({ paths, onAction, onClose }: RemoveAllConfirmProps) {
  const [isRemoving, setIsRemoving] = useState(false);
  const noun = paths.length === 1 ? "worktree" : "worktrees";

  const removeAll = async () => {
    setIsRemoving(true);
    // Stop at the first failure so its error stays in the alert line instead of being cleared
    // by the next removal.
    for (const path of paths) {
      if ((await onAction("remove-now", path)) === false) break;
    }
    setIsRemoving(false);
    onClose();
  };

  // Escape closes the confirm only; it must not reach the popover's own Escape, which hides it.
  const cancelOnEscape = (event: KeyboardEvent) => {
    if (event.key !== "Escape") return;
    event.stopPropagation();
    onClose();
  };

  return (
    <div role="group" aria-label="Confirm remove all" onKeyDown={cancelOnEscape} className="py-1">
      <Callout tone="plain" heading={<span className="font-medium">{`Remove ${paths.length} ${noun}?`}</span>}>
        <p className="text-muted">git still checks each one and refuses any that changed.</p>
        <div className="flex gap-2">
          <TextButton variant="inset" disabled={isRemoving} onClick={() => void removeAll()}>
            {isRemoving ? "Removing…" : "Remove"}
          </TextButton>
          <TextButton variant="inset" autoFocus disabled={isRemoving} onClick={onClose}>
            Cancel
          </TextButton>
        </div>
      </Callout>
    </div>
  );
}
