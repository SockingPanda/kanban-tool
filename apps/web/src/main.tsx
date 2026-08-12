import "./styles.css"

import { bootstrapWebApp, renderRuntimeStartupError } from "./bootstrap"
import { prepareWebPreferences } from "./lib/preferences"

const root = document.getElementById("root")

if (!root) {
  throw new Error("Missing #root element")
}

prepareWebPreferences()
void bootstrapWebApp(root).catch((error: unknown) => renderRuntimeStartupError(root, error))
