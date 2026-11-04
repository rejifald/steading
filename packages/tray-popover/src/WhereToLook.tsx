import { useId } from "react";

import { CrossIcon, FolderIcon } from "./controls/icons";
import { IconButton } from "./controls/IconButton";
import { TextButton } from "./controls/TextButton";

export interface WhereToLookRoot {
  /** The folder, as shown (`~/Development`). */
  path: string;
  /** What the last scan found there ("5 clones"); absent when it has not run. */
  found?: string | undefined;
  /** Why the last scan could not read it; shown in place of `found`, in the needs-you colour. */
  error?: string | undefined;
}

export interface WhereToLookSkippedClone {
  /** The clone's folder; the key, and the tooltip on the name and root. */
  path: string;
  name: string;
  /** The scan root it lies under; null when none does. */
  root: string | null;
}

interface WhereToLookProps {
  /** "Folders". */
  foldersTitle: string;
  roots: WhereToLookRoot[];
  /** *Add folder…*: the tool opens its folder picker. */
  addFolderLabel: string;
  onAddRoot: () => void;
  onRemoveRoot: (path: string) => void;
  /** The ✕'s tooltip and accessible name for a root ("Stop looking in ~/Code"). */
  removeRootLabel: (path: string) => string;
  /** The protected-folders sentence under the folders. */
  note: string;
  /** "Skipped"; the whole section is hidden while `skipped` is empty. */
  skippedTitle: string;
  skipped: WhereToLookSkippedClone[];
  unskipLabel: string;
  onUnskip: (path: string) => void;
}

const ROW = "flex h-8 items-center gap-2 px-2";

function SkippedRow({
  clone,
  unskipLabel,
  onUnskip,
}: {
  clone: WhereToLookSkippedClone;
  unskipLabel: string;
  onUnskip: (path: string) => void;
}) {
  const nameId = useId();
  return (
    <li className={ROW}>
      <span id={nameId} title={clone.path} className="truncate font-medium">
        {clone.name}
      </span>
      <span title={clone.path} className="flex-1 truncate text-faint">
        {clone.root}
      </span>
      <TextButton size="compact" variant="inset" onClick={() => onUnskip(clone.path)} aria-describedby={nameId}>
        {unskipLabel}
      </TextButton>
    </li>
  );
}

/**
 * The body of the "Where to look" view: the folders a scan walks and the clones skipped. The
 * buttons that leave the view are `WhereToLookFooter`.
 */
export function WhereToLook({
  foldersTitle,
  roots,
  addFolderLabel,
  onAddRoot,
  onRemoveRoot,
  removeRootLabel,
  note,
  skippedTitle,
  skipped,
  unskipLabel,
  onUnskip,
}: WhereToLookProps) {
  const foldersId = useId();
  const skippedId = useId();
  return (
    <>
      <section aria-labelledby={foldersId}>
        <div className="flex items-center gap-2 px-4 pt-3 pb-1">
          <h2 id={foldersId} className="text-sm font-semibold text-text">
            {foldersTitle}
          </h2>
          <span className="ml-auto">
            {/* The view's first control, so the focus starts here. */}
            <TextButton size="compact" variant="quiet" onClick={onAddRoot} autoFocus>
              {addFolderLabel}
            </TextButton>
          </span>
        </div>
        {roots.length > 0 && (
          <ul aria-labelledby={foldersId} className="px-2 pb-1">
            {roots.map((root) => (
              <li key={root.path} className={ROW}>
                <FolderIcon className="shrink-0 text-muted" />
                <span title={root.path} className="flex-1 truncate font-medium">
                  {root.path}
                </span>
                {(root.error ?? root.found) !== undefined && (
                  <span
                    title={root.error}
                    className={`shrink-0 text-xs ${root.error !== undefined ? "text-needs-you" : "text-faint"}`}
                  >
                    {root.error ?? root.found}
                  </span>
                )}
                <IconButton label={removeRootLabel(root.path)} onClick={() => onRemoveRoot(root.path)}>
                  <CrossIcon />
                </IconButton>
              </li>
            ))}
          </ul>
        )}
        <p className="px-4 pb-2 text-xs text-muted">{note}</p>
      </section>
      {skipped.length > 0 && (
        <section aria-labelledby={skippedId} className="border-t border-border">
          <div className="flex items-center gap-2 px-4 pt-3 pb-1">
            <h2 className="flex gap-2 text-sm font-semibold text-text">
              <span id={skippedId}>{skippedTitle}</span> <span className="font-normal text-faint">{skipped.length}</span>
            </h2>
          </div>
          <ul aria-labelledby={skippedId} className="px-2 pb-1">
            {skipped.map((clone) => (
              <SkippedRow key={clone.path} clone={clone} unskipLabel={unskipLabel} onUnskip={onUnskip} />
            ))}
          </ul>
        </section>
      )}
    </>
  );
}

interface WhereToLookFooterProps {
  /** *Done*: back to the list. */
  doneLabel: string;
  onDone: () => void;
  /** *Look again*: leave the view and scan. */
  lookAgainLabel: string;
  onLookAgain: () => void;
  /** True with no folder left to look in. */
  lookAgainDisabled?: boolean | undefined;
}

/** The view's footer buttons, for the tool to put in a `PopoverFooter`: *Done* quiet, *Look again* primary at the right. */
export function WhereToLookFooter({
  doneLabel,
  onDone,
  lookAgainLabel,
  onLookAgain,
  lookAgainDisabled = false,
}: WhereToLookFooterProps) {
  return (
    <>
      <TextButton variant="quiet" onClick={onDone}>
        {doneLabel}
      </TextButton>
      <TextButton variant="primary" className="ml-auto" onClick={onLookAgain} disabled={lookAgainDisabled}>
        {lookAgainLabel}
      </TextButton>
    </>
  );
}
