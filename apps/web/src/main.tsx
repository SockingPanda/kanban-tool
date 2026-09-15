
import { bootstrapWebApp, renderRuntimeStartupError } from "./bootstrap"
import { prepareWebPreferences } from "./platform/preferences/preferences"
import "./layers.css"
import "./app/styles.css"

const root = document.getElementById("root")

if (!root) {
  throw new Error("Missing #root element")
}

prepareWebPreferences()
void bootstrapWebApp(root).catch((error: unknown) => renderRuntimeStartupError(root, error))
