import { useState } from 'react';
import { Dialog } from "../../components/ui/dialog";
import { IconButton } from "../../components/ui/button";
import { cn } from "../../components/ui/classes";
import { usePreferences } from "../../platform/preferences/use-preferences";
import { createTranslator } from "../../application/i18n";
import { ProjectNavigation } from "./project-navigation";
import { RouteContent } from "./route-content";
import type { ProductShellProps } from "./shell-contract";
export type { ProductShellProps, ShellBoundary } from './shell-contract';

const pageLabels: Record<string, string> = { board: '看板', list: '任务列表', map: '依赖图', runs: '运行记录', events: '项目动态', health: '健康', maintenance: '维护', settings: '设置' };

export function ProductShell(props: ProductShellProps) {
  const { runtime, route, canonicalBoardSlug, onNavigate, syncStatus } = props;
  const preferences = usePreferences();
  const t = createTranslator(preferences.locale);
  const [mobileNav, setMobileNav] = useState(false);
  const board = canonicalBoardSlug ?? ('boardSlug' in route ? route.boardSlug : undefined);
  const page = route.kind === 'board' ? route.view ?? 'board' : route.kind;
  return <div className={cn('app-shell paper-visual-shell', !preferences.sidebarExpanded && 'is-sidebar-collapsed')} data-testid="product-shell">
    <a href="#main-content" className="skip-link">跳到主要内容</a>
    <aside className="desktop-sidebar" id="desktop-project-navigation" data-testid="persistent-sidebar">
      <ProjectNavigation runtime={runtime} route={route} canonicalBoardSlug={board} onNavigate={onNavigate} />
    </aside>
    <div className="main-shell">
      <header className="workspace-header">
        <div className="workspace-breadcrumb">
          <IconButton className="mobile-nav-toggle" icon="panel" label="展开项目导航" onClick={() => setMobileNav(true)} />
          <span className="breadcrumb-project">{board ?? 'Kanban Tool'}</span><span className="breadcrumb-separator">/</span>
          <strong>{pageLabels[page] ?? t('productName')}</strong>
        </div>
        <div className="workspace-actions"><span className="project-privacy">个人工作空间</span>
          <IconButton icon={preferences.theme === 'dark' ? 'sun' : 'moon'} label="切换浅色与深色" onClick={() => preferences.setTheme(preferences.theme === 'dark' ? 'light' : 'dark')} />
        </div>
      </header>
      <main className="main-content" id="main-content" tabIndex={-1} aria-label={t('productName')}
        data-runtime-api-base-url={runtime.apiBaseUrl} data-runtime-actor={runtime.actor}
        data-runtime-default-board={runtime.defaultBoard} data-runtime-server-version={runtime.serverVersion}
        data-runtime-protocol-version={runtime.protocolVersion} data-runtime-web-build-id={runtime.webBuildId}
        data-runtime-web-base-path={runtime.webBasePath}>
        <RouteContent {...props} />
      </main>
      <footer className="workspace-status"><span><span className={cn('online-dot', syncStatus !== 'live' && 'connection-pending')} />{syncStatus === 'live' ? 'Host 已连接' : syncStatus === 'offline' ? '连接已断开' : syncStatus === 'stale' ? '正在重新同步' : '连接中'}</span><span>Kanban Tool · {runtime.serverVersion}</span></footer>
    </div>
    <div id="global-detail-root" className="global-detail-root" />
    <Dialog open={mobileNav} onClose={() => setMobileNav(false)} title="项目导航" className="mobile-navigation-dialog">
      <ProjectNavigation runtime={runtime} route={route} canonicalBoardSlug={board} onNavigate={target => { setMobileNav(false); return onNavigate?.(target); }} mobile />
    </Dialog>
  </div>;
}
