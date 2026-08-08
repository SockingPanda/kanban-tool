import { StrictMode } from "react"
import { createRoot } from "react-dom/client"

import "@astryxdesign/core/reset.css"
import "@astryxdesign/core/astryx.css"
import "@astryxdesign/theme-neutral/theme.css"

import App from "./App"
import "./layers.css"
import "./styles.css"

const root = document.getElementById("root")

if (!root) {
  throw new Error("Missing #root element")
}

createRoot(root).render(
  <StrictMode>
    <App />
  </StrictMode>,
)
