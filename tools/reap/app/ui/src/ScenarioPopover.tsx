import { useState } from "react";

import { PopoverWindow } from "@steading/tray-popover/storybook/decorators";

import { createFakeApi, type FakeOptions } from "./api.fake";
import { SCENARIOS } from "./api.scenarios";
import { Popover } from "./Popover";

interface ScenarioPopoverProps {
  /** A key of `SCENARIOS`, as `/?scenario=<name>` takes it. */
  scenario: string;
}

/**
 * Storybook only: the popover against its own fake API for one scenario, in a window-sized
 * frame. The app never imports this file.
 */
export function ScenarioPopover({ scenario }: ScenarioPopoverProps) {
  // A new scenario (the Controls panel) gets a new fake API, not the old one's state.
  return <ScenarioFrame key={scenario} options={SCENARIOS[scenario] ?? {}} />;
}

function ScenarioFrame({ options }: { options: FakeOptions }) {
  const [api] = useState(() => createFakeApi(options));
  return (
    <PopoverWindow>
      <Popover api={api} />
    </PopoverWindow>
  );
}
