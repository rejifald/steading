import type { Preview } from "@storybook/react-vite";
import { SHARED_PREVIEW } from "@steading/tray-popover/storybook/decorators";

import "../src/theme.css";

const preview: Preview = { ...SHARED_PREVIEW };

export default preview;
