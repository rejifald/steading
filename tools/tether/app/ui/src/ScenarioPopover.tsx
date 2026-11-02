import { useState } from "react";

import { createFakeApi, type FakeOptions } from "./api.fake";
import { SCENARIOS } from "./api.scenarios";
import { Popover } from "./Popover";

/** The tray window's size: 360 wide, at most 480 tall (docs/design/popover/anatomy.md § 1). */
const WINDOW = { width: 360, height: 480 } as const;

interface ScenarioPopoverProps {
  /** A key of `SCENARIOS`, as `/?scenario=<name>` takes it. */
  scenario: string;
  /** Fields that replace the scenario's own, for a board that differs in one value. */
  options?: FakeOptions;
}

/**
 * Storybook only: the popover against its own fake API for one scenario, in a window-sized
 * frame. The app never imports this file.
 */
export function ScenarioPopover(props: ScenarioPopoverProps) {
  // A new scenario (the Controls panel) gets a new fake API, not the old one's state.
  return <ScenarioFrame key={props.scenario} {...props} />;
}

function ScenarioFrame({ scenario, options }: ScenarioPopoverProps) {
  const [api] = useState(() => createFakeApi({ ...SCENARIOS[scenario], ...options }));
  return (
    <div style={WINDOW}>
      <Popover api={api} />
    </div>
  );
}
