import { Select } from "@base-ui/react/select";

import { CheckIcon, ChevronDownIcon } from "@steading/tray-popover/controls/icons";
import type { IntervalChoice } from "./intervals";

interface IntervalSelectProps {
  /** The id of the visible label that names this control. */
  labelledBy: string;
  choices: IntervalChoice[];
  value: string;
  onValueChange: (value: string) => void;
  disabled?: boolean;
}

/** *Pull every*: how often tether polls a repo, from a short list of presets. */
export function IntervalSelect({
  labelledBy,
  choices,
  value,
  onValueChange,
  disabled = false,
}: IntervalSelectProps) {
  const items = choices.map((choice) => ({ value: choice.value, label: choice.label }));
  return (
    <Select.Root
      items={items}
      value={value}
      disabled={disabled}
      onValueChange={(next) => next !== null && onValueChange(next)}
    >
      <Select.Trigger
        aria-labelledby={labelledBy}
        className="flex h-7 items-center justify-between gap-2 rounded-control border border-border bg-surface-raised px-2 text-sm text-text hover:bg-surface-hover focus-visible:outline-2 focus-visible:-outline-offset-1 focus-visible:outline-focus data-disabled:opacity-50"
      >
        <Select.Value />
        <Select.Icon className="text-muted">
          <ChevronDownIcon />
        </Select.Icon>
      </Select.Trigger>
      <Select.Portal>
        {/* Below the trigger and as wide as it, not laid over it on the selected item. */}
        <Select.Positioner alignItemWithTrigger={false} side="bottom" sideOffset={4} className="z-10">
          <Select.Popup
            style={{ width: "var(--anchor-width)" }}
            className="rounded-control border border-border bg-surface-raised p-1 text-sm text-text shadow-raised"
          >
            <Select.List>
              {items.map((item) => (
                <Select.Item
                  key={item.value}
                  value={item.value}
                  className="flex h-7 cursor-default items-center gap-2 rounded-control px-2 outline-none data-highlighted:bg-surface-hover data-selected:font-medium"
                >
                  <Select.ItemText>{item.label}</Select.ItemText>
                  <Select.ItemIndicator className="ml-auto text-muted">
                    <CheckIcon />
                  </Select.ItemIndicator>
                </Select.Item>
              ))}
            </Select.List>
          </Select.Popup>
        </Select.Positioner>
      </Select.Portal>
    </Select.Root>
  );
}
