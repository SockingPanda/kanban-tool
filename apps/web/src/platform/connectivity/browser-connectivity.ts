import { createContext, useContext } from "react"

/** The shell owns the browser connectivity listener; feature routes consume the same value. */
export const browserConnectivityContext = createContext<boolean | undefined>(undefined)

export function useBrowserOnline(fallback = typeof navigator === "undefined" || navigator.onLine): boolean {
  return useContext(browserConnectivityContext) ?? fallback
}
