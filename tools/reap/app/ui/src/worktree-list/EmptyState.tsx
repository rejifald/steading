import { FoundPreview } from "@steading/tray-popover/FoundSection";
import { TextButton } from "@steading/tray-popover/controls/TextButton";

import type { Discovery } from "../api";
import { lookedLineOf, pathsOf, rootsText } from "../discovery/copy";

/** The made-up clones in the invitation's faded box. */
const PREVIEW_ROWS = [
  { name: "your-app", detail: "7 worktrees" },
  { name: "your-site", detail: "2 worktrees" },
];

type EmptyStateProps = {
  discovery: Discovery;
  now: Date;
  onAdd: () => void;
  onWhereToLook: () => void;
} & (
  | {
      /** Before any scan: the invitation to find clones. */
      variant: "invite";
      onFind: () => void;
    }
  | {
      /** After a scan that found nothing, with nothing listed. */
      variant: "found-nothing";
    }
);

const BODY = "flex flex-1 flex-col items-center justify-center gap-4 p-6";

/** The body when no repository is listed: the invitation to find clones, or what a scan found (nothing). */
export function EmptyState(props: EmptyStateProps) {
  const { discovery, onAdd, onWhereToLook } = props;
  const roots = pathsOf(discovery.roots);
  if (props.variant === "found-nothing") {
    return (
      <div className={BODY}>
        <p className="text-center text-sm text-muted">
          {roots.length > 0
            ? `No clones with worktrees in ${rootsText(roots)}. Look somewhere else, or add one by hand.`
            : "No folders to look in. Add one, or add a repository by hand."}
        </p>
        <div className="flex gap-2">
          <TextButton variant="quiet" onClick={onWhereToLook}>
            Where to look…
          </TextButton>
          <TextButton variant="primary" onClick={onAdd}>
            Add repository
          </TextButton>
        </div>
        <p className="text-xs text-faint">{lookedLineOf(discovery, props.now)}</p>
      </div>
    );
  }
  return (
    <div className={BODY}>
      <FoundPreview rows={PREVIEW_ROWS} addLabel="Add" />
      <p className="text-center text-sm text-muted">
        reap removes the worktrees whose work has merged. Find the clones on this Mac that have worktrees, or add one
        by hand.
      </p>
      <div className="flex gap-2">
        <TextButton variant="quiet" onClick={onAdd}>
          Add repository
        </TextButton>
        <TextButton variant="primary" onClick={props.onFind}>
          Find clones
        </TextButton>
      </div>
      <p className="flex items-center gap-2 text-xs text-faint">
        {roots.length > 0 ? `Looks in ${rootsText(roots)}.` : "Looks nowhere yet."}
        <TextButton size="compact" variant="link" onClick={onWhereToLook}>
          Where to look…
        </TextButton>
      </p>
    </div>
  );
}
