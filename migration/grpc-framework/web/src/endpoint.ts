/** Atlas 的 strict CSP 和共享 Web artifact 要求同源 RPC；不放宽到任意 loopback 端口。 */
export function atlasRpcEndpoint(input: string, documentUrl: string): string {
  const page = new URL(documentUrl)
  const url = new URL(input, page)
  if (!["http:", "https:"].includes(url.protocol) || url.origin !== page.origin
    || !["127.0.0.1", "localhost", "[::1]"].includes(url.hostname)
    || url.username || url.password || url.search || url.hash
    || input.includes("\\") || /%2f|%5c|%00/i.test(input)
    || input.split("/").some(part => part === "." || part === ".." || /%2e/i.test(part))) {
    throw new Error("Atlas RPC 必须使用同源、无凭据的本地 endpoint")
  }
  return url.href.replace(/\/$/, "")
}
