import { TextButton } from "@steading/tray-popover/controls/TextButton";

import { RepoLine } from "../RepoLine";

interface EmptyStateProps {
  onAdd: () => void;
}

/** The body when no repository is listed yet: a ghost preview, one line of copy, the way in. */
export function EmptyState({ onAdd }: EmptyStateProps) {
  return (
    <div className="flex flex-1 flex-col items-center justify-center gap-4 p-6">
      <div className="w-full rounded-control border border-dashed border-border py-1 opacity-50">
        <RepoLine ghost name="your-app" branch="main" mode="follow" health="in-sync" />
        <RepoLine ghost name="your-docs" branch="main" mode="mirror" health="behind" behind={3} />
      </div>
      <p className="text-center text-sm text-muted">
        tether keeps each clone you list on its branch, pulling in the background.
      </p>
      <TextButton variant="primary" onClick={onAdd}>
        Add repository
      </TextButton>
    </div>
  );
}
