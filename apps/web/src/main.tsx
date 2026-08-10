import "@astryxdesign/core/reset.css"
import "@astryxdesign/core/astryx.css"
import "@astryxdesign/theme-neutral/theme.css"

import { bootstrapWebApp, renderRuntimeStartupError } from "./bootstrap"
import { prepareWebPreferences } from "./lib/preferences"
import "./layers.css"
import "./styles.css"

const root = document.getElementById("root")

if (!root) {
  throw new Error("Missing #root element")
}

prepareWebPreferences()
void bootstrapWebApp(root).catch((error: unknown) => renderRuntimeStartupError(root, error))
