import { expect, userEvent, within } from "storybook/test";

import { DISMISSED_KEY } from "./repo-list/useDismissed";
import { SCENARIOS } from "./api.scenarios";

/**
 * Storybook only: the clicks and keys that bring a scenario to a drawn state the data alone
 * does not reach (a form open, focus on a control, a menu open). The app never imports this.
 */

/** Most keys any drawn focus state is from the start of the window. */
const MAX_TABS = 20;

const page = (canvasElement: HTMLElement) => within(canvasElement.ownerDocument.body);

/** Opens the add form from the footer and, unless `choose` is false, picks the folder. */
export async function openAddForm(canvasElement: HTMLElement, { choose = true } = {}) {
  const canvas = within(canvasElement);
  await userEvent.click(await canvas.findByRole("button", { name: "Add repository" }));
  if (choose) await userEvent.click(await canvas.findByRole("button", { name: "Choose…" }));
}

/** Presses *Find clones* in the invitation: the scan starts and Found fills in per root. */
export async function findClones(canvasElement: HTMLElement) {
  await userEvent.click(await within(canvasElement).findByRole("button", { name: "Find clones" }));
}

/** Opens the "Where to look" view from the invitation, the Found heading or the nothing-found state. */
export async function openWhereToLook(canvasElement: HTMLElement) {
  await userEvent.click(await within(canvasElement).findByRole("button", { name: "Where to look…" }));
  await within(canvasElement).findByRole("heading", { name: "Where to look", level: 1 });
}

/** Opens the collapsed Found row under the listed repos. */
export async function expandFound(canvasElement: HTMLElement) {
  await userEvent.click(await within(canvasElement).findByRole("button", { name: /^Found\s*\d/ }));
}

/** Opens a repo's settings from its row, or from its callout when it needs you. */
export async function openSettings(canvasElement: HTMLElement, name: string) {
  const canvas = within(canvasElement);
  const callout = canvas.queryByRole("region", { name: `${name} needs you` });
  const holder = callout ?? (await canvas.findByRole("listitem", { name }));
  await userEvent.click(within(holder).getByRole("button", { name: "Settings" }));
  await canvas.findByRole("form", { name: `${name} settings` });
}

/**
 * Opens the *Pull every* menu from the keyboard: in a real browser Base UI's select does not
 * stay open after a synthetic click, whose press and release land in the same instant.
 */
export async function openIntervalMenu(canvasElement: HTMLElement) {
  within(canvasElement).getByRole("combobox").focus();
  await userEvent.keyboard("{ArrowDown}");
  await page(canvasElement).findByRole("listbox");
}

/** Picks a *Pull every* choice in the open form. */
export async function chooseInterval(canvasElement: HTMLElement, label: string) {
  await openIntervalMenu(canvasElement);
  await userEvent.click(await page(canvasElement).findByRole("option", { name: label }));
}

/** Presses Tab until `target` has focus, as a keyboard user gets there. */
export async function tabTo(target: HTMLElement) {
  for (let i = 0; i < MAX_TABS && target.ownerDocument.activeElement !== target; i++) {
    await userEvent.tab();
  }
  await expect(target).toHaveFocus();
}

/** Dismisses repos as the × does, against the reason text a scenario sends for each. */
export function dismissIn(scenario: string, paths: string[]) {
  const details = SCENARIOS[scenario]?.details ?? {};
  const dismissed = Object.fromEntries(paths.map((path) => [path, details[path] ?? ""]));
  localStorage.setItem(DISMISSED_KEY, JSON.stringify(dismissed));
}
