import type { Preview } from "@storybook/react-vite";
import { SHARED_PREVIEW } from "@steading/tray-popover/storybook/decorators";

import { DISMISSED_KEY } from "../src/repo-list/useDismissed";
import "../src/theme.css";

const preview: Preview = {
  ...SHARED_PREVIEW,
  // Dismissals live in localStorage; every story starts with none unless it sets its own.
  beforeEach: () => {
    localStorage.removeItem(DISMISSED_KEY);
  },
};

export default preview;
