import path from "node:path"
import tailwindcss from "@tailwindcss/vite"
import react from "@vitejs/plugin-react"
import { defineConfig, loadEnv, type ProxyOptions } from "vite"

const DEFAULT_DEV_API_BASE = "/__kb_api__"
const DEFAULT_DEV_PROXY_TARGET = "http://127.0.0.1:8721"

export default defineConfig(({ mode }) => {
  const env = loadEnv(mode, __dirname, "")
  const proxy = devApiProxy(
    env.VITE_KB_API_BASE_URL || DEFAULT_DEV_API_BASE,
    env.VITE_KB_DEV_PROXY_TARGET || DEFAULT_DEV_PROXY_TARGET,
  )

  return {
    plugins: [react(), tailwindcss()],
    resolve: {
      alias: {
        "@": path.resolve(__dirname, "./src"),
      },
    },
    server: {
      host: "127.0.0.1",
      port: 1420,
      strictPort: true,
      ...(proxy ? { proxy } : {}),
    },
    build: {
      rollupOptions: {
        output: {
          manualChunks(id) {
            if (id.includes("node_modules/elkjs")) return "graph-elk"
            if (id.includes("node_modules/@xyflow")) return "graph-flow"
            if (id.includes("node_modules/react-markdown") || id.includes("node_modules/remark-gfm")) return "markdown-renderer"
            if (
              id.includes("node_modules/react/") ||
              id.includes("node_modules/react-dom/") ||
              id.includes("node_modules/react-is/") ||
              id.includes("node_modules/scheduler/")
            ) {
              return "react-core"
            }
            if (id.includes("node_modules/@radix-ui") || id.includes("node_modules/lucide-react")) return "ui-vendor"
            if (id.includes("node_modules/@tanstack")) return "query-table-vendor"
          },
        },
      },
    },
  }
})

function devApiProxy(apiBaseUrl: string | undefined, target: string | undefined) {
  const base = apiBaseUrl?.trim().replace(/\/+$/, "")
  const proxyTarget = target?.trim()
  if (!base || !proxyTarget || !base.startsWith("/") || base === "/") return undefined

  return {
    [base]: {
      target: proxyTarget,
      changeOrigin: false,
      rewrite: (requestPath: string) => requestPath.slice(base.length) || "/",
    } satisfies ProxyOptions,
  }
}
