import type { ReactNode } from 'react';
import type { WorkspaceDataSource } from './data-source';
import { WorkspaceDataSourceContext } from './data-source-context';
export function WorkspaceDataSourceProvider({ source, children }: { source: WorkspaceDataSource; children: ReactNode }) {
  return <WorkspaceDataSourceContext value={source}>{children}</WorkspaceDataSourceContext>;
}
