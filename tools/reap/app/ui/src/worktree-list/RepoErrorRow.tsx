import type { Repo, WorktreeAction } from "../api";
import { TextButton } from "@steading/tray-popover/controls/TextButton";

interface RepoErrorRowProps {
  repo: Pick<Repo, "name" | "path" | "error">;
  onAction: (action: WorktreeAction, path: string) => void;
}

/** A repository reap could not read, as a Needs-you row: the repo's name and the error. */
export function RepoErrorRow({ repo, onAction }: RepoErrorRowProps) {
  return (
    <li aria-label={repo.name} className="flex flex-col gap-0.5 px-4 py-1.5 hover:bg-surface-hover">
      <span className="truncate font-medium text-text">{repo.name}</span>
      <p className="text-xs text-needs-you">{repo.error}</p>
      <div className="flex gap-2 pt-1">
        <TextButton variant="inset" onClick={() => onAction("open", repo.path)}>
          Open in Finder
        </TextButton>
      </div>
    </li>
  );
}
