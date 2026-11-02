import type { Repo, Worktree, WorktreeAction } from "../api";
import { BinIcon, FolderIcon, PinIcon } from "../controls/icons";
import { IconButton } from "../controls/IconButton";
import { TextButton } from "../controls/TextButton";
import { shownSize } from "../shownSize";
import { ago, inTime } from "./ago";
import { sentenceOf } from "./sentenceOf";

interface WorktreeRowProps {
  worktree: Worktree;
  repo: Pick<Repo, "name" | "policy">;
  now: Date;
  onAction: (action: WorktreeAction, path: string) => void;
  /** A Needs-you row: the sentence wraps and the buttons are text buttons below it. */
  emphasis?: boolean;
}

const ACTIONS_HIDDEN_UNTIL_HOVER =
  "flex w-0 shrink-0 gap-1 overflow-hidden opacity-0 group-hover:w-auto group-hover:opacity-100 group-focus-within:w-auto group-focus-within:opacity-100";

/** The branch, or the folder name when HEAD is detached. */
export function titleOf(worktree: Pick<Worktree, "branch" | "name">): string {
  return worktree.branch ?? worktree.name;
}

/** "3 d" for a known last activity; nothing when it is unknown. */
function ageOf(worktree: Worktree, now: Date): string | null {
  if (!worktree.last_activity) return null;
  return ago(worktree.last_activity, now).replace(/ ago$/, "");
}

function descriptionOf(worktree: Worktree, now: Date): string {
  const removal = worktree.removes_at ? ` · removes ${inTime(worktree.removes_at, now)}` : "";
  const size = worktree.size_bytes === undefined ? "" : ` · takes ${shownSize(worktree.size_bytes)}`;
  return `${worktree.name} · ${worktree.reason}${removal} · last activity ${ago(worktree.last_activity, now)}${size}`;
}

/** "git-reap · 3 d · 840 MB": the repo, then the age and size where known. */
function metaOf(worktree: Worktree, repo: Pick<Repo, "name">, now: Date): string {
  const size = worktree.size_bytes === undefined ? null : shownSize(worktree.size_bytes);
  return [repo.name, ageOf(worktree, now), size].filter((part) => part !== null).join(" · ");
}

function RowActions({ worktree, onAction }: Pick<WorktreeRowProps, "worktree" | "onAction">) {
  const { state, path } = worktree;
  // A missing worktree is cleared by a sweep; the core refuses to remove it by hand.
  const canRemove = state === "removable";
  const canKeep = canRemove || state === "dirty" || state === "in-use" || state === "locked";
  return (
    <div className={ACTIONS_HIDDEN_UNTIL_HOVER}>
      {canRemove && (
        <IconButton label="Remove now" onClick={() => onAction("remove-now", path)}>
          <BinIcon />
        </IconButton>
      )}
      {canKeep && (
        <IconButton label="Keep" onClick={() => onAction("keep", path)}>
          <PinIcon />
        </IconButton>
      )}
      {state === "kept" && (
        <IconButton label="Unkeep" onClick={() => onAction("unkeep", path)}>
          <PinIcon />
        </IconButton>
      )}
      <IconButton label="Open in Finder" onClick={() => onAction("open", path)}>
        <FolderIcon />
      </IconButton>
    </div>
  );
}

/** One worktree as two lines: the branch and where it lives, then what happens to it. */
export function WorktreeRow({ worktree, repo, now, onAction, emphasis = false }: WorktreeRowProps) {
  const meta = metaOf(worktree, repo, now);
  const sentence = sentenceOf(worktree, repo, now);
  return (
    <li
      aria-label={titleOf(worktree)}
      aria-description={descriptionOf(worktree, now)}
      className="group flex flex-col gap-0.5 px-4 py-1.5 text-sm hover:bg-surface-hover focus-within:bg-surface-hover"
    >
      <div className="flex items-center gap-2">
        <span className="min-w-0 flex-1 truncate font-medium text-text">{titleOf(worktree)}</span>
        {!emphasis && <RowActions worktree={worktree} onAction={onAction} />}
        <span className="shrink-0 text-xs text-faint">{meta}</span>
      </div>
      {emphasis ? (
        <>
          <p className="text-xs text-needs-you">{sentence}</p>
          <div className="flex gap-2 pt-1">
            {worktree.state === "refused" && (
              <>
                <TextButton variant="inset" onClick={() => onAction("remove-now", worktree.path)}>
                  Try again
                </TextButton>
                <TextButton variant="inset" onClick={() => onAction("keep", worktree.path)}>
                  Keep
                </TextButton>
              </>
            )}
            <TextButton variant="inset" onClick={() => onAction("open", worktree.path)}>
              Open in Finder
            </TextButton>
          </div>
        </>
      ) : (
        <p className="truncate text-xs text-muted">{sentence}</p>
      )}
    </li>
  );
}
