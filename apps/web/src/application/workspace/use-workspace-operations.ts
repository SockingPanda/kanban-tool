import { useContext } from 'react';
import { WorkspaceDataSourceContext } from './data-source-context';
export function useWorkspaceOperations() {
  const source = useContext(WorkspaceDataSourceContext);
  if (!source) throw new Error('应用启动时必须提供数据源。');
  return source;
}
