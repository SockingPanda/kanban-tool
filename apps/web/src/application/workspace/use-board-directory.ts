import { useAsyncRead } from '../query/use-async-read';
import { useWorkspaceOperations } from './use-workspace-operations';
import { useWebRuntime } from '../../lib/runtime-context';
import { runtimeIdentityKey } from './board-session-registry';
export function useBoardDirectory() {
  const source = useWorkspaceOperations();
  const runtime = useWebRuntime();
  return useAsyncRead(true, runtimeIdentityKey(runtime) + '|boards', signal => source.readBoardDirectory(signal));
}
