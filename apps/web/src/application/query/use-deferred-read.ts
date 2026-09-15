import { useEffect, useLayoutEffect, useRef, useState } from 'react';

export type DeferredReadStatus = 'idle' | 'loading' | 'ready' | 'stale' | 'offline' | 'error';
interface Snapshot<T> { identity: string; data: T; hasData: boolean; status: DeferredReadStatus; revision: number }

function sectionStatus<T>(snapshot: Snapshot<T>, open: boolean, online: boolean, revision: number): DeferredReadStatus {
  if (!online) return snapshot.hasData ? 'stale' : 'offline';
  if (!open && snapshot.status === 'loading') return snapshot.hasData ? 'stale' : 'idle';
  if (snapshot.status === 'ready' && snapshot.revision !== revision) return 'stale';
  return snapshot.status;
}

/** 折叠区按需读取；关闭时中止请求，缓存按 identity 隔离，重连后刷新已展开区域。 */
export function useDeferredRead<T>(identity: string, initial: T, hasInitial: boolean, load: ((signal: AbortSignal) => Promise<T>) | undefined, revision: number, online: boolean) {
  const [open, setOpen] = useState(false);
  const [attempt, setAttempt] = useState(0);
  const [snapshot, setSnapshot] = useState<Snapshot<T>>(() => ({ identity, data: initial, hasData: hasInitial, status: hasInitial ? 'ready' : 'idle', revision }));
  const completedRequest = useRef<string | null>(null);
  const available = load !== undefined;
  const inputs = useRef({ load, initial, hasInitial });
  useLayoutEffect(() => { inputs.current = { load, initial, hasInitial }; });
  useEffect(() => {
    const { load: reader, initial: seed, hasInitial: hasSeed } = inputs.current;
    if (!online) { completedRequest.current = null; return; }
    if (!open || !reader) return;
    const request = JSON.stringify([identity, revision, attempt]);
    if (completedRequest.current === request) return;
    const controller = new AbortController();
    let active = true;
    setSnapshot(current => ({ identity, data: current.identity === identity ? current.data : seed, hasData: current.identity === identity ? current.hasData : hasSeed, status: 'loading', revision }));
    void reader(controller.signal).then(data => {
      if (active) { completedRequest.current = request; setSnapshot({ identity, data, hasData: true, status: 'ready', revision }); }
    }, () => {
      if (active) setSnapshot(current => ({ ...current, status: 'error' }));
    });
    return () => { active = false; controller.abort(); };
  }, [attempt, available, identity, online, open, revision]);
  const current = snapshot.identity === identity ? snapshot : { identity, data: initial, hasData: hasInitial, status: hasInitial ? 'ready' as const : 'idle' as const, revision };
  return { data: current.data, status: sectionStatus(current, open, online, revision), setOpen, retry: () => setAttempt(value => value + 1) };
}
