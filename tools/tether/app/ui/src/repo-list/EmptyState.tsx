import { TextButton } from "@steading/tray-popover/controls/TextButton";
import { FoundPreview } from "@steading/tray-popover/FoundSection";

import type { Discovery } from "../api";
import { agoInWords } from "./ago";
import { errorLinesOf, rootsText, searchedOf } from "./discoveryCopy";

/** The folder budget of a scan (find-clones' `Bounds::default`), the cause the copy names. */
const FOLDER_CAP = "20 000";

interface EmptyStateProps {
  /** `invite`: before any scan. `found-nothing`: a scan ran and nothing is left to suggest. */
  variant: "invite" | "found-nothing";
  discovery: Discovery;
  now: Date;
  onAdd: () => void;
  onFind: () => void;
  onWhereToLook: () => void;
}

/** The body when no repository is listed and no suggestion is showing. */
export function EmptyState({ variant, discovery, now, onAdd, onFind, onWhereToLook }: EmptyStateProps) {
  const roots = discovery.roots.map((root) => root.path);
  return (
    <div className="flex flex-1 flex-col items-center justify-center gap-4 p-6">
      {variant === "invite" ? (
        <Invite roots={roots} onAdd={onAdd} onFind={onFind} onWhereToLook={onWhereToLook} />
      ) : (
        <FoundNothing discovery={discovery} now={now} onAdd={onAdd} onWhereToLook={onWhereToLook} />
      )}
    </div>
  );
}

function Invite({
  roots,
  onAdd,
  onFind,
  onWhereToLook,
}: { roots: string[] } & Pick<EmptyStateProps, "onAdd" | "onFind" | "onWhereToLook">) {
  return (
    <>
      <FoundPreview
        addLabel="Add"
        rows={[
          { name: "your-app", detail: "main · ~/Development" },
          { name: "your-docs", detail: "main · ~/Code" },
        ]}
      />
      <p className="text-center text-sm text-muted">
        tether keeps each clone you list on its branch. Find the clones on this Mac, or add one by hand.
      </p>
      <div className="flex gap-2">
        <TextButton variant="quiet" onClick={onAdd}>
          Add repository
        </TextButton>
        <TextButton variant="primary" onClick={onFind} disabled={roots.length === 0}>
          Find clones
        </TextButton>
      </div>
      <p className="flex items-center gap-2 text-xs text-faint">
        {roots.length === 0 ? "Looks nowhere yet." : `Looks in ${rootsText(roots)}.`}
        <TextButton variant="link" size="compact" onClick={onWhereToLook}>
          Where to look…
        </TextButton>
      </p>
    </>
  );
}

function FoundNothing({
  discovery,
  now,
  onAdd,
  onWhereToLook,
}: { discovery: Discovery; now: Date } & Pick<EmptyStateProps, "onAdd" | "onWhereToLook">) {
  const searched = searchedOf(discovery);
  const stopped = discovery.roots.filter((root) => root.stopped_early).map((root) => root.path);
  const errors = errorLinesOf(discovery);
  const looked = `Looked ${agoInWords(discovery.scanned_at ?? undefined, now)}`;
  return (
    <>
      <p className="text-center text-sm text-muted">
        {searched.length === 0 ? "No clones found." : `No clones in ${rootsText(searched, "or")}.`} Look somewhere
        else, or add one by hand.
      </p>
      <div className="flex gap-2">
        <TextButton variant="quiet" onClick={onWhereToLook}>
          Where to look…
        </TextButton>
        <TextButton variant="primary" onClick={onAdd}>
          Add repository
        </TextButton>
      </div>
      <p className="text-center text-xs text-faint">
        {looked}
        {stopped.length > 0 && ` · stopped early in ${rootsText(stopped)} after ${FOLDER_CAP} folders`}.
      </p>
      {errors.map((line) => (
        <p key={line} className="text-center text-xs text-faint">
          {line}
        </p>
      ))}
    </>
  );
}
