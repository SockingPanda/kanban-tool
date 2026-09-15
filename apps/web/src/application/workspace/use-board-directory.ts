import { useSyncExternalStore } from 'react';
import { useAsyncRead } from '../query/use-async-read';
import { useWorkspaceOperations } from './use-workspace-operations';
import { boardSessionRevision, subscribeBoardSessions } from './board-session-registry';
export function useBoardDirectory() {
  const source = useWorkspaceOperations();
  const revision = useSyncExternalStore(subscribeBoardSessions, boardSessionRevision, boardSessionRevision);
  return useAsyncRead(true, (source.boardRealtime?.key ?? source.streamUrl ?? '') + '|boards', signal => source.readBoardDirectory(signal), revision);
}
