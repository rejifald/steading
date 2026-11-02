import type { SVGProps } from "react";

type IconProps = SVGProps<SVGSVGElement>;

const base = {
  width: 16,
  height: 16,
  viewBox: "0 0 16 16",
  fill: "none",
  stroke: "currentColor",
  strokeWidth: 1.5,
  strokeLinecap: "round",
  strokeLinejoin: "round",
  "aria-hidden": true,
} as const;

/** Two arrows chasing each other: *Sync now*. */
export function SyncIcon(props: IconProps) {
  return (
    <svg {...base} {...props}>
      <path d="M13 6.5A5 5 0 0 0 4 4.2L3 5.5" />
      <path d="M3 2.5v3h3" />
      <path d="M3 9.5a5 5 0 0 0 9 2.3l1-1.3" />
      <path d="M13 13.5v-3h-3" />
    </svg>
  );
}

/** A folder: *Open in Finder*. */
export function FolderIcon(props: IconProps) {
  return (
    <svg {...base} {...props}>
      <path d="M2 4.5A1.5 1.5 0 0 1 3.5 3h2.6l1.4 1.5h5A1.5 1.5 0 0 1 14 6v5.5a1.5 1.5 0 0 1-1.5 1.5h-9A1.5 1.5 0 0 1 2 11.5z" />
    </svg>
  );
}

/** Three sliders: a repo's settings. */
export function SettingsIcon(props: IconProps) {
  return (
    <svg {...base} {...props}>
      <path d="M2.5 4h3.5M9.5 4h4M2.5 8h7.5M2.5 12h1.5M7.5 12h6" />
      <circle cx="7.75" cy="4" r="1.5" />
      <circle cx="11.75" cy="8" r="1.5" />
      <circle cx="5.75" cy="12" r="1.5" />
    </svg>
  );
}

/** A small down chevron: a select's trigger. */
export function ChevronIcon(props: IconProps) {
  return (
    <svg {...base} width={12} height={12} {...props}>
      <path d="M4 6l4 4 4-4" />
    </svg>
  );
}

/** A check in a circle: in sync. */
export function InSyncIcon(props: IconProps) {
  return (
    <svg {...base} {...props}>
      <circle cx="8" cy="8" r="6.25" />
      <path d="M5.4 8.2l1.8 1.8 3.4-3.7" />
    </svg>
  );
}

/** A down arrow into a line: behind. */
export function BehindIcon(props: IconProps) {
  return (
    <svg {...base} {...props}>
      <path d="M8 2.75v7.5M4.75 7.25L8 10.5l3.25-3.25M3.5 13.25h9" />
    </svg>
  );
}

/** A minus in a circle: skipped. */
export function SkippedIcon(props: IconProps) {
  return (
    <svg {...base} {...props}>
      <circle cx="8" cy="8" r="6.25" />
      <path d="M5.25 8h5.5" />
    </svg>
  );
}

/** A warning triangle: needs you. */
export function NeedsYouIcon(props: IconProps) {
  return (
    <svg {...base} {...props}>
      <path d="M8 2.25l6.25 11H1.75z" />
      <path d="M8 6.5v3M8 11.4v.1" />
    </svg>
  );
}

/** An open arc, spun by its caller: syncing. */
export function SyncingIcon(props: IconProps) {
  return (
    <svg {...base} {...props}>
      <path d="M13.25 8A5.25 5.25 0 1 1 8 2.75" />
    </svg>
  );
}

/** Two bars in a circle: paused. */
export function PausedIcon(props: IconProps) {
  return (
    <svg {...base} {...props}>
      <circle cx="8" cy="8" r="6.25" />
      <path d="M6.5 5.75v4.5M9.5 5.75v4.5" />
    </svg>
  );
}

/** A dashed circle: a repo not listed yet. */
export function PendingIcon(props: IconProps) {
  return (
    <svg {...base} {...props}>
      <circle cx="8" cy="8" r="6.25" strokeDasharray="2 2.4" />
    </svg>
  );
}

/** A cross: *Dismiss until it changes*. */
export function DismissIcon(props: IconProps) {
  return (
    <svg {...base} {...props}>
      <path d="M4.5 4.5l7 7M11.5 4.5l-7 7" />
    </svg>
  );
}

/** A bare check: the selected menu item. */
export function CheckIcon(props: IconProps) {
  return (
    <svg {...base} {...props}>
      <path d="M4 8.5l2.5 2.5 5.5-6" />
    </svg>
  );
}
