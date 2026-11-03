import type { KeyboardEvent } from "react";
import { useState } from "react";

import type { WorktreeAction } from "../api";
import { TextButton } from "@steading/tray-popover/controls/TextButton";

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
 * The confirm under the Ready heading, in place of its lead line: a full-width row, since the
 * heading has no room for it at 360px.
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
    <div role="group" aria-label="Confirm remove all" onKeyDown={cancelOnEscape} className="flex items-center gap-2 py-1">
      <span className="flex-1 text-sm text-text">{`Remove ${paths.length} ${noun}?`}</span>
      <TextButton size="compact" autoFocus disabled={isRemoving} onClick={onClose}>
        Cancel
      </TextButton>
      <TextButton variant="primary" size="compact" disabled={isRemoving} onClick={() => void removeAll()}>
        Remove
      </TextButton>
    </div>
  );
}
