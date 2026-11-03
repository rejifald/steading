import { Button } from "@base-ui/react/button";
import { Tooltip } from "@base-ui/react/tooltip";
import type { ReactNode } from "react";

interface IconButtonProps {
  /** The tooltip and the accessible name, always the same words. */
  label: string;
  onClick: () => void;
  busy?: boolean;
  children: ReactNode;
}

/** An icon-only action with a tooltip; its `aria-label` equals the tooltip. */
export function IconButton({ label, onClick, busy = false, children }: IconButtonProps) {
  return (
    <Tooltip.Root>
      <Tooltip.Trigger
        render={<Button focusableWhenDisabled />}
        aria-label={label}
        aria-busy={busy}
        disabled={busy}
        onClick={onClick}
        className="flex size-6 items-center justify-center rounded-control text-muted hover:bg-surface-hover hover:text-text focus-visible:outline-2 focus-visible:outline-focus data-disabled:text-faint"
      >
        {children}
      </Tooltip.Trigger>
      <Tooltip.Portal>
        <Tooltip.Positioner side="bottom" sideOffset={4}>
          <Tooltip.Popup className="rounded-control border border-border bg-surface-raised px-2 py-1 text-xs text-text shadow-raised">
            {label}
          </Tooltip.Popup>
        </Tooltip.Positioner>
      </Tooltip.Portal>
    </Tooltip.Root>
  );
}
