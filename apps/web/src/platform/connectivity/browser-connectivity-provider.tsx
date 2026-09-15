import { type ReactNode } from "react"

import { browserConnectivityContext } from "./browser-connectivity"

export function BrowserConnectivityProvider({ online, children }: { readonly online: boolean; readonly children: ReactNode }) {
  return <browserConnectivityContext.Provider value={online}>{children}</browserConnectivityContext.Provider>
}
