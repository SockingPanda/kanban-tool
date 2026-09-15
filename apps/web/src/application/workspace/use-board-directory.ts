import { useAsyncRead } from '../query/use-async-read';
import { useWorkspaceOperations } from './use-workspace-operations';
export function useBoardDirectory() {
  const source = useWorkspaceOperations();
  return useAsyncRead(true, source.streamUrl + '|boards', signal => source.readBoardDirectory(signal));
}
