import { createContext } from 'react';
import type { WorkspaceDataSource } from './data-source';
export const WorkspaceDataSourceContext = createContext<WorkspaceDataSource | null>(null);
