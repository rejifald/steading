import { Tooltip } from "@base-ui/react/tooltip";
import type { ReactNode } from "react";

/** How long a pointer rests before a tooltip opens. */
const TOOLTIP_DELAY = 400;

/** The panel the tray window shows: header, body and footer in a rounded column, at most the window's height. */
export function PopoverFrame({ children }: { children: ReactNode }) {
  return (
    <Tooltip.Provider delay={TOOLTIP_DELAY}>
      <main className="flex h-full max-h-full flex-col overflow-hidden rounded-panel border border-border bg-surface text-sm text-text">
        {children}
      </main>
    </Tooltip.Provider>
  );
}
