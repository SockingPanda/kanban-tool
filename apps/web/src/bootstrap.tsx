import { StrictMode, type ReactNode } from "react"
import { createRoot, type Root } from "react-dom/client"

import App from "./App"
import styles from "./foundation.module.css"
import { WebRuntimeProvider } from "./lib/runtime-provider"
import {
  loadWebRuntimeConfig,
  runtimeErrorMessage,
  type WebRuntimeConfig,
} from "./lib/runtime"

export type BootstrapDependencies = {
  loadRuntime?: () => Promise<WebRuntimeConfig>
  mount?: (root: HTMLElement, runtime: WebRuntimeConfig) => void
}

function renderApp(runtime: WebRuntimeConfig): ReactNode {
  return (
    <StrictMode>
      <WebRuntimeProvider runtime={runtime}>
        <App />
      </WebRuntimeProvider>
    </StrictMode>
  )
}

export function mountWebApp(root: HTMLElement, runtime: WebRuntimeConfig): Root {
  const reactRoot = createRoot(root)
  reactRoot.render(renderApp(runtime))
  return reactRoot
}

/** 只有 runtime 校验成功后才创建 React root 并 mount 产品 App。 */
export async function bootstrapWebApp(root: HTMLElement, dependencies: BootstrapDependencies = {}) {
  const runtime = await (dependencies.loadRuntime ?? (() => loadWebRuntimeConfig()))()
  if (dependencies.mount) dependencies.mount(root, runtime)
  else mountWebApp(root, runtime)
  return runtime
}

export function renderRuntimeStartupError(root: HTMLElement, error: unknown): Root {
  const reactRoot = createRoot(root)
  reactRoot.render(
    <main className={styles.startupError} role="alert" aria-live="assertive" data-testid="runtime-startup-error">
      <p className={styles.eyebrow}>KANBAN TOOL / RUNTIME</p>
      <h1>Kanban Tool 无法启动</h1>
      <p>{runtimeErrorMessage(error)}</p>
      <p className={styles.muted}>请检查 kanban serve 与当前 Web artifact 是否来自同一版本，然后重新加载页面。</p>
    </main>,
  )
  return reactRoot
}
