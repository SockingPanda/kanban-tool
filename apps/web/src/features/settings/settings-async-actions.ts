/** Normalize sync and async Settings seams so pending state always settles on throw. */
export function callSettingsAction<T>(operation: () => T | PromiseLike<T>): Promise<T> {
  return Promise.resolve().then(operation)
}
