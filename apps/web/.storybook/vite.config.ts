import path from "node:path"

import tailwindcss from "@tailwindcss/vite"
import { defineConfig } from "vite"

import { createContractValidatorPlugin } from "../build/contract-validator.ts"
import { createUiGuardPlugin, uiGuardModeFromEnv } from "../build/ui-guard.ts"

/**
 * Storybook-only Vite config. Production's `vite.config.ts` is intentionally
 * not loaded: it owns the `/app/` artifact, strict preview CSP, and manifest.
 * Page stories still import production presentation owners whose generated
 * read-model modules require the same static contract validator transform.
 */
export default defineConfig({
  root: path.resolve(import.meta.dirname, ".."),
  plugins: [tailwindcss(), createUiGuardPlugin({ mode: uiGuardModeFromEnv() }), createContractValidatorPlugin()],
  resolve: {
    alias: {
      "@": path.resolve(import.meta.dirname, "../src"),
    },
  },
})
