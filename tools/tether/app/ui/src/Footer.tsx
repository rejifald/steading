import { TextButton } from "./controls/TextButton";

interface FooterProps {
  hasRepos: boolean;
  /** Before the first report: the full footer, with Sync all disabled. */
  isLoading?: boolean;
  /** A sync is running: Sync all is disabled. */
  isSyncing?: boolean;
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
  paused,
  onSyncAll,
  onTogglePause,
  onAdd,
}: FooterProps) {
  return (
    <footer className="flex items-center gap-2 border-t border-border px-4 py-3">
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
    </footer>
  );
}
