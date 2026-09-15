import type { ReactNode } from 'react';
import { renderToStaticMarkup as render } from 'react-dom/server';
import { WorkspaceDataSourceProvider } from '../../src/application/workspace/data-source-provider';
import { createHostDataSource } from '../../src/adapters/host/data-source';
import type { WebRuntimeConfig } from '../../src/lib/runtime';

const runtime: WebRuntimeConfig = { apiBaseUrl: '', actor: 'render-test', defaultBoard: 'default', protocolVersion: '1', serverVersion: '3.1.0', webBasePath: '/app/', webBuildId: 'render-test' };
/** SSR 只验证显示内容；effect 不运行，操作仍使用显式 Host 端口。 */
export function renderToStaticMarkup(node: ReactNode): string {
  return render(<WorkspaceDataSourceProvider source={createHostDataSource(runtime)}>{node}</WorkspaceDataSourceProvider>);
}
