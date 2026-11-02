import { render } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import {
  BehindIcon,
  CheckIcon,
  DismissIcon,
  InSyncIcon,
  NeedsYouIcon,
  PausedIcon,
  PendingIcon,
  SkippedIcon,
  SyncingIcon,
} from "./icons";

const ICONS = { InSyncIcon, BehindIcon, SkippedIcon, NeedsYouIcon, SyncingIcon, PausedIcon, PendingIcon, DismissIcon, CheckIcon };

describe("icons", () => {
  it.each(Object.entries(ICONS))("%s is hidden from assistive technology", (_, Icon) => {
    const { container } = render(<Icon />);
    expect(container.querySelector("svg")).toHaveAttribute("aria-hidden", "true");
  });
});
