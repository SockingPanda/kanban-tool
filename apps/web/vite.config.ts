import path from "node:path"

import react from "@vitejs/plugin-react"
import { defineConfig } from "vite"

const strictPreviewCsp =
  "default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self' data:; font-src 'self'; connect-src 'self'; object-src 'none'; base-uri 'self'; frame-ancestors 'none'"

export default defineConfig({
  base: "/app/",
  plugins: [
    react(),
    {
      name: "strict-preview-csp",
      configurePreviewServer(server) {
        server.middlewares.use((_request, response, next) => {
          response.setHeader("Content-Security-Policy", strictPreviewCsp)
          next()
        })
      },
    },
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
  },
})
