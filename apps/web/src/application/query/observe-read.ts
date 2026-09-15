/** 一个依赖只持有 registry 的读取能力；业务数据和 cursor 由 transport 统一拥有。 */
export interface ReadDependency<T> {
  readonly key: string
  readonly version: () => number
  readonly read: () => Promise<{ readonly value: T; readonly version: number }>
  readonly subscribe: (changed: () => void) => () => void
  readonly retry: () => void
}

interface DependencyUse {
  readonly dependency: ReadDependency<unknown>
  readonly release: () => void
  used: number
  version: number
}

interface ReadScope {
  read<T>(dependency: ReadDependency<T>): Promise<T>
}

const scopes = new WeakMap<AbortSignal, ReadScope>()

/** adapter 合并取消信号时显式传递读取作用域，保证复合读取共享同一组依赖。 */
export function inheritReadScope(parent: AbortSignal | undefined, child: AbortSignal): void {
  const scope = parent && scopes.get(parent)
  if (scope) scopes.set(child, scope)
}

export function readObservedDependency<T>(signal: AbortSignal | undefined, dependency: ReadDependency<T>): Promise<T> {
  const scope = signal && scopes.get(signal)
  if (scope) return scope.read(dependency)
  const release = dependency.subscribe(() => undefined)
  const abort = () => release()
  signal?.addEventListener('abort', abort, { once: true })
  return dependency.read().then(snapshot => snapshot.value).finally(() => {
    signal?.removeEventListener('abort', abort)
    release()
  })
}

/**
 * 订阅本次 mapper 实际读取的 typed 查询。仅相关查询提交后重新映射快照；没有网络回读或全局失效。
 * 异步映射期间若依赖又发生变化，丢弃该轮结果。分支改变后释放旧依赖，卸载释放全部引用。
 */
export function observeRead<T>(
  load: (signal: AbortSignal) => Promise<T>,
  signal: AbortSignal,
  next: (value: T) => void,
  failed: (error: unknown) => void,
  refresh = false,
): void {
  const dependencies = new Map<string, DependencyUse>()
  let iteration = 0
  let running = false
  let queued = false
  const changed = () => {
    if (signal.aborted || running || queued) return
    queued = true
    queueMicrotask(() => { queued = false; void run() })
  }
  const scope: ReadScope = {
    async read(dependency) {
      let use = dependencies.get(dependency.key)
      if (!use) {
        use = { dependency, release: dependency.subscribe(changed), used: iteration, version: dependency.version() }
        dependencies.set(dependency.key, use)
        if (refresh) dependency.retry()
      }
      use.used = iteration
      try {
        const snapshot = await dependency.read()
        use.version = snapshot.version
        return snapshot.value
      } catch (error) { use.version = dependency.version(); throw error }
    },
  }
  scopes.set(signal, scope)
  const stop = () => {
    scopes.delete(signal)
    for (const use of dependencies.values()) use.release()
    dependencies.clear()
  }
  signal.addEventListener('abort', stop, { once: true })
  async function run(): Promise<void> {
    if (signal.aborted || running) return
    running = true
    iteration += 1
    let value: T | undefined
    let error: unknown
    let succeeded = false
    try { value = await load(signal); succeeded = true }
    catch (cause) { error = cause }
    if (signal.aborted) { running = false; return }
    let current = true
    for (const [key, use] of dependencies) {
      if (use.used !== iteration) { use.release(); dependencies.delete(key) }
      else if (use.version !== use.dependency.version()) current = false
    }
    running = false
    if (!current) { changed(); return }
    if (succeeded) next(value as T)
    else failed(error)
  }
  if (signal.aborted) stop()
  else void run()
}
