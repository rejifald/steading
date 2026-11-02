import { Radio } from "@base-ui/react/radio";

interface ModeOptionProps {
  value: string;
  label: string;
  hint: string;
  /** Whether this option is the group's value; draws the accent border, raised. */
  checked: boolean;
}

/** One bordered radio row of the add form's mode group: a label and what the mode does. */
export function ModeOption({ value, label, hint, checked }: ModeOptionProps) {
  const tone = checked ? "border-accent bg-surface-raised shadow-raised" : "border-border";
  return (
    <label
      className={`flex items-start gap-2 rounded-control border p-2 text-sm text-text has-data-disabled:opacity-50 ${tone}`}
    >
      <Radio.Root
        value={value}
        className="mt-0.5 flex size-4 shrink-0 items-center justify-center rounded-full border border-border bg-surface-raised focus-visible:outline-2 focus-visible:outline-offset-1 focus-visible:outline-focus data-checked:border-accent data-checked:bg-accent"
      >
        <Radio.Indicator className="size-2 rounded-full bg-accent-text data-unchecked:hidden" />
      </Radio.Root>
      <span>
        {label}
        <span className="block text-xs text-muted">{hint}</span>
      </span>
    </label>
  );
}
