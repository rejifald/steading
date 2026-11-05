import { TextButton } from "@steading/tray-popover/controls/TextButton";
import { PopoverFooter } from "@steading/tray-popover/PopoverFooter";

interface FooterProps {
  hasRepos: boolean;
  /** Before the first report: the full footer, with Sync all disabled. */
  isLoading?: boolean;
  /** A sync is running: Sync all is disabled. */
  isSyncing?: boolean;
  /** Nothing is listed but the Found section fills the body: *Add repository* sits at the right, quiet. */
  isFoundBody?: boolean;
  /** Resume becomes the primary action and Add repository goes quiet. */
  paused: boolean;
  onSyncAll: () => void;
  onTogglePause: () => void;
  onAdd: () => void;
}

/** The popover's fixed actions. */
export function Footer({
  hasRepos,
  isLoading = false,
  isSyncing = false,
  isFoundBody = false,
  paused,
  onSyncAll,
  onTogglePause,
  onAdd,
}: FooterProps) {
  return (
    <PopoverFooter>
      {(hasRepos || isLoading) && (
        <TextButton disabled={isLoading || isSyncing} onClick={onSyncAll}>
          Sync all
        </TextButton>
      )}
      <TextButton variant={paused ? "primary" : "quiet"} onClick={onTogglePause}>
        {paused ? "Resume" : "Pause"}
      </TextButton>
      {(hasRepos || isLoading) && (
        <TextButton variant={paused ? "quiet" : "primary"} className="ml-auto" onClick={onAdd}>
          Add repository
        </TextButton>
      )}
      {isFoundBody && !hasRepos && (
        <TextButton variant="quiet" className="ml-auto" onClick={onAdd}>
          Add repository
        </TextButton>
      )}
    </PopoverFooter>
  );
}
