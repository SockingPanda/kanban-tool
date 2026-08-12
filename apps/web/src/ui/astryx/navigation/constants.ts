import type { TreeLevel } from "./types"

/** Finite level-to-utility mapping used by the CSP-safe tree renderer. */
export const TREE_LEVEL_CLASSES: Readonly<Record<TreeLevel, string>> = {
  1: "pl-2",
  2: "pl-4",
  3: "pl-6",
  4: "pl-8",
  5: "pl-10",
  6: "pl-12",
}

/** Finite connector utilities; each supported depth has an explicit bridge class. */
export const TREE_GUIDE_CLASSES: Readonly<Record<TreeLevel, string>> = {
  1: "ml-4 border-l border-border",
  2: "ml-6 border-l border-border",
  3: "ml-8 border-l border-border",
  4: "ml-10 border-l border-border",
  5: "ml-12 border-l border-border",
  6: "ml-12 border-l border-border",
}
