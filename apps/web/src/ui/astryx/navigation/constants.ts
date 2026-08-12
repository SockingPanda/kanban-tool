import type { TreeLevel } from "./types"

/** Finite level-to-utility mapping used by the CSP-safe tree renderer. */
export const TREE_LEVEL_CLASSES: Readonly<Record<TreeLevel, string>> = {
  1: "ps-2",
  2: "ps-4",
  3: "ps-6",
  4: "ps-8",
  5: "ps-10",
  6: "ps-12",
}

/** Finite connector utilities; each supported depth has an explicit bridge class. */
export const TREE_GUIDE_CLASSES: Readonly<Record<TreeLevel, string>> = {
  1: "ms-4 border-s border-border",
  2: "ms-6 border-s border-border",
  3: "ms-8 border-s border-border",
  4: "ms-10 border-s border-border",
  5: "ms-12 border-s border-border",
  6: "ms-12 border-s border-border",
}
