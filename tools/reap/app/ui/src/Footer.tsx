import { TextButton } from "@steading/tray-popover/controls/TextButton";
import { PopoverFooter } from "@steading/tray-popover/PopoverFooter";

interface FooterProps {
  hasRepos: boolean;
  /** Nothing is listed but Found fills the body: *Add repository* stays, quiet. */
  hasFound?: boolean;
  /** Before the first report: the full footer, with Refresh disabled. */
  isLoading?: boolean;
  isRefreshing?: boolean;
  /** Resume becomes the primary action and Add repository goes quiet. */
  paused: boolean;
  onRefresh: () => void;
  onTogglePause: () => void;
  onAdd: () => void;
}

/** The popover's fixed actions. */
export function Footer({
  hasRepos,
  hasFound = false,
  isLoading = false,
  isRefreshing = false,
  paused,
  onRefresh,
  onTogglePause,
  onAdd,
}: FooterProps) {
  const showsRepoActions = hasRepos || isLoading;
  return (
    <PopoverFooter>
      {showsRepoActions && (
        <TextButton disabled={isLoading || isRefreshing} onClick={onRefresh}>
          Refresh
        </TextButton>
      )}
      <TextButton variant={paused ? "primary" : "quiet"} onClick={onTogglePause}>
        {paused ? "Resume" : "Pause"}
      </TextButton>
      {(showsRepoActions || hasFound) && (
        <TextButton variant={paused || !showsRepoActions ? "quiet" : "primary"} className="ml-auto" onClick={onAdd}>
          Add repository
        </TextButton>
      )}
    </PopoverFooter>
  );
}
