import path from "node:path"

import { defineConfig } from "vite"

/**
 * Storybook-only Vite config. Production's `vite.config.ts` is intentionally
 * not loaded: it owns the `/app/` artifact, strict preview CSP, and manifest.
 */
export default defineConfig({
  root: path.resolve(import.meta.dirname, ".."),
  resolve: {
    alias: {
      "@": path.resolve(import.meta.dirname, "../src"),
    },
  },
})
