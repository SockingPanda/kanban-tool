export interface BrowserConnectivityTarget {
  addEventListener(type: "online" | "offline", listener: () => void): void
  removeEventListener(type: "online" | "offline", listener: () => void): void
}

/** Register connectivity listeners even while the board read is bootstrapping. */
export function subscribeBrowserConnectivity(
  target: BrowserConnectivityTarget,
  onOffline: () => void,
  onOnline: () => void,
): () => void {
  target.addEventListener("offline", onOffline)
  target.addEventListener("online", onOnline)
  return () => {
    target.removeEventListener("offline", onOffline)
    target.removeEventListener("online", onOnline)
  }
}
