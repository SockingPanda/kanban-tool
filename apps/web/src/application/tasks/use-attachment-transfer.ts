import { useMemo } from 'react';
import type { WebRuntimeConfig } from '../../lib/runtime';
import { useWorkspaceOperations } from '../workspace/use-workspace-operations';

export function useAttachmentTransfer(runtime: WebRuntimeConfig, boardSelector: string) {
  const source = useWorkspaceOperations();
  return useMemo(() => source.createAttachmentTransferClient(runtime, boardSelector), [source, runtime, boardSelector]);
}
