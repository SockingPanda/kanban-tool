export function isCurrentHealthRequest(
  controller: AbortController,
  activeController: AbortController | null,
): boolean {
  return activeController === controller && !controller.signal.aborted
}
