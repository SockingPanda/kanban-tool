import { readFileSync } from "node:fs"
import path from "node:path"

import react from "@vitejs/plugin-react"
import { defineConfig, loadEnv } from "vite"

import { createWebArtifactManifestPlugin } from "./build/artifact-manifest.ts"
import { createContractValidatorPlugin } from "./build/contract-validator.ts"
import { createKanbanHostProxy } from "./build/host-proxy.ts"

const webPackage = JSON.parse(
  readFileSync(new URL("./package.json", import.meta.url), "utf8"),
) as { version?: unknown }
if (typeof webPackage.version !== "string") {
  throw new Error("apps/web/package.json must declare a string version")
}
const webPackageVersion = webPackage.version

const strictPreviewCsp =
  "default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self' data:; font-src 'self'; connect-src 'self'; object-src 'none'; base-uri 'self'; frame-ancestors 'none'"

export default defineConfig(({ mode }) => {
  const proxy = createKanbanHostProxy(loadEnv(mode, import.meta.dirname, "KANBAN_").KANBAN_HOST_URL)
  return {
    base: "/app/",
    plugins: [
      react(),
      createContractValidatorPlugin(),
      {
        name: "strict-preview-csp",
        configurePreviewServer(server) {
          server.middlewares.use((_request, response, next) => {
            response.setHeader("Content-Security-Policy", strictPreviewCsp)
            next()
          })
        },
      },
      createWebArtifactManifestPlugin({ serverVersion: webPackageVersion }),
    ],
    resolve: {
      alias: {
        "@": path.resolve(import.meta.dirname, "./src"),
      },
    },
    server: {
      host: "127.0.0.1",
      port: 1421,
      strictPort: true,
      cors: false,
      proxy,
    },
    preview: {
      host: "127.0.0.1",
      cors: false,
      proxy,
    },
    build: {
      emptyOutDir: true,
    },
  }
})
