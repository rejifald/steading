import { TextButton } from "./controls/TextButton";

interface FooterProps {
  hasRepos: boolean;
  /** Before the first report: the full footer, with Refresh disabled. */
  isLoading?: boolean;
  isRefreshing?: boolean;
  paused: boolean;
  onRefresh: () => void;
  onTogglePause: () => void;
  onAdd: () => void;
}

/** The popover's fixed actions. */
export function Footer({
  hasRepos,
  isLoading = false,
  isRefreshing = false,
  paused,
  onRefresh,
  onTogglePause,
  onAdd,
}: FooterProps) {
  const showsRepoActions = hasRepos || isLoading;
  return (
    <footer className="flex items-center gap-2 border-t border-border px-4 py-3">
      {showsRepoActions && (
        <TextButton disabled={isLoading || isRefreshing} onClick={onRefresh}>
          Refresh
        </TextButton>
      )}
      <TextButton onClick={onTogglePause}>{paused ? "Resume" : "Pause"}</TextButton>
      {showsRepoActions && (
        <TextButton variant="primary" className="ml-auto" onClick={onAdd}>
          Add repository
        </TextButton>
      )}
    </footer>
  );
}
