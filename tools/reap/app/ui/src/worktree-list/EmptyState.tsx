import { TextButton } from "@steading/tray-popover/controls/TextButton";
import { WorktreeLine } from "./WorktreeLine";

interface EmptyStateProps {
  onAdd: () => void;
}

/** The body when no repository is listed yet: a ghost preview, one line of copy, the way in. */
export function EmptyState({ onAdd }: EmptyStateProps) {
  return (
    <div className="flex flex-1 flex-col items-center justify-center gap-4 p-6">
      <div className="w-full rounded-control border border-dashed border-border py-1 opacity-50">
        <WorktreeLine ghost name="fix-login" detail="fix/login · #41" state="removable" />
        <WorktreeLine ghost name="new-api" detail="feat/api" state="open" />
      </div>
      <p className="text-center text-sm text-muted">
        reap removes the worktrees whose work has merged, once nothing runs inside them.
      </p>
      <TextButton variant="primary" onClick={onAdd}>
        Add repository
      </TextButton>
    </div>
  );
}
