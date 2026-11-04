import { TextButton } from "@steading/tray-popover/controls/TextButton";

import type { Repo, WorktreeAction } from "../api";
import { NeedsYouCallout } from "./NeedsYouCallout";

interface RepoErrorRowProps {
  repo: Pick<Repo, "name" | "path" | "error">;
  onAction: (action: WorktreeAction, path: string) => void;
}

/** A repository reap could not read, as a Needs-you callout: the repo's name and the error. */
export function RepoErrorRow({ repo, onAction }: RepoErrorRowProps) {
  return (
    <li aria-label={repo.name}>
      <NeedsYouCallout title={repo.name} sentence={repo.error ?? ""}>
        <TextButton variant="inset" onClick={() => onAction("open", repo.path)}>
          Open in Finder
        </TextButton>
      </NeedsYouCallout>
    </li>
  );
}
