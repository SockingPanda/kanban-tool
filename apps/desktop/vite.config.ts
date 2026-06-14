import path from "node:path"
import tailwindcss from "@tailwindcss/vite"
import react from "@vitejs/plugin-react"
import { defineConfig, loadEnv, type ProxyOptions } from "vite"

export default defineConfig(({ mode }) => {
  const env = loadEnv(mode, __dirname, "")
  const proxy = devApiProxy(env.VITE_KB_API_BASE_URL, env.VITE_KB_DEV_PROXY_TARGET)

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
