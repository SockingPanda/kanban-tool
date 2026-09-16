import { useCallback, useEffect, useLayoutEffect, useRef, useState } from 'react';
import type { AttachmentTransferClient } from '../data/attachment-transfer';
import { AttachmentQueue, type UploadEntry } from './attachment-queue';

export function useAttachmentQueue(taskId: string, client: AttachmentTransferClient | null, reconcile: () => Promise<void>, onBusyChange?: (busy: boolean) => void) {
  const reconcileRef = useRef(reconcile);
  const busyRef = useRef(onBusyChange);
  useLayoutEffect(() => { busyRef.current = onBusyChange; }, [onBusyChange]);
  useLayoutEffect(() => { reconcileRef.current = reconcile; }, [reconcile]);
  const queueRef = useRef<AttachmentQueue | null>(null);
  const [entries, setEntries] = useState<readonly UploadEntry[]>([]);
  // effect 内创建可释放资源，支持 StrictMode 的 setup / cleanup / setup。
  useEffect(() => {
    if (!client) return;
    const queue = new AttachmentQueue({ taskId, transport: client, reconcile: () => reconcileRef.current() });
    queueRef.current = queue;
    setEntries(queue.getSnapshot());
    const unsubscribe = queue.subscribe(() => { const next=queue.getSnapshot(); setEntries(next); busyRef.current?.(next.some(item=>['queued','uploading','committing'].includes(item.status))); });
    return () => { unsubscribe(); queue.dispose(); busyRef.current?.(false); if (queueRef.current === queue) queueRef.current = null; };
  }, [taskId, client]);
  return {
    entries,
    enqueue: useCallback((files: Iterable<File>) => queueRef.current?.enqueue(files) ?? [{ code: 'file.not_ready' }], []),
    cancel: useCallback((id: string) => queueRef.current?.cancel(id), []),
    retry: useCallback((id: string) => queueRef.current?.retry(id), []),
    clear: useCallback(() => queueRef.current?.clearSettled(), []),
    reconcile: useCallback(async () => { if (queueRef.current) await queueRef.current.reconcile(); else await reconcileRef.current(); }, []),
  };
}
