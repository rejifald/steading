import { ScrollArea } from "@base-ui/react/scroll-area";
import type { ReactNode } from "react";

/** The scrolling middle of the popover: the header and footer never scroll. */
export function ListScroll({ children }: { children: ReactNode }) {
  return (
    <ScrollArea.Root className="relative min-h-0 flex-1">
      <ScrollArea.Viewport className="h-full focus-visible:outline-2 focus-visible:-outline-offset-2 focus-visible:outline-focus">
        {children}
      </ScrollArea.Viewport>
      <ScrollArea.Scrollbar className="flex w-2 justify-center py-1 opacity-0 data-hovering:opacity-100 data-scrolling:opacity-100">
        <ScrollArea.Thumb className="w-full rounded-full bg-border" />
      </ScrollArea.Scrollbar>
    </ScrollArea.Root>
  );
}
