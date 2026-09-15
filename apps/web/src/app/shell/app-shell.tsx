import { useResolvedTheme } from '../../platform/preferences/use-resolved-theme';
import { useEffect, useState } from 'react';
import { Dialog } from "../../components/ui/dialog";
import { Button, IconButton } from "../../components/ui/button";
import { Icon } from '../../components/ui/icon';
import { cn } from "../../components/ui/classes";
import { usePreferences } from "../../platform/preferences/use-preferences";
import { createTranslator } from "../../application/i18n";
import { ProjectNavigation } from "./project-navigation";
import { RouteContent } from "./route-content";
import type { ProductShellProps } from "./shell-contract";
export type { ProductShellProps, ShellBoundary } from './shell-contract';

const pageLabels: Record<string, string> = { board: '任务', list: '任务', map: '依赖图', runs: '运行记录', events: '项目动态', health: '健康', maintenance: '维护', settings: '设置' };

export function ProductShell(props: ProductShellProps) {
  const { runtime, route, canonicalBoardSlug, onNavigate, syncStatus } = props;
  const preferences = usePreferences();
  const resolvedTheme = useResolvedTheme();
  const t = createTranslator(preferences.locale);
  const [mobileNav, setMobileNav] = useState(false);
  const board = canonicalBoardSlug ?? ('boardSlug' in route ? route.boardSlug : undefined);
  const page = route.kind === 'board' ? route.view ?? 'board' : route.kind;
  useEffect(() => {
    const shortcut = (event: KeyboardEvent) => {
      if (!board || event.defaultPrevented || document.querySelector('dialog:modal, [data-choice-popup]')) return;
      const editing = event.target instanceof HTMLElement && (event.target.matches('input, textarea, select') || event.target.isContentEditable);
      if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 'k') {
        event.preventDefault();
        const input = document.querySelector<HTMLInputElement>('[data-testid="list-search"]');
        if (input) input.focus();
        else void onNavigate?.({kind:'board',boardSlug:board,view:'list',query:'focus=search'});
      } else if (!editing && !event.ctrlKey && !event.metaKey && !event.altKey && event.key.toLowerCase() === 'c') {
        event.preventDefault();
        void onNavigate?.({kind:'board',boardSlug:board,view:'list',query:'create=1'});
      }
    };
    window.addEventListener('keydown', shortcut);
    return () => window.removeEventListener('keydown', shortcut);
  }, [board, onNavigate]);
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
        <div className="workspace-actions"><span className="project-privacy"><Icon name="lock" size={12} />个人项目</span>
          <IconButton icon="search" label="搜索任务与能力" onClick={() => { if (board) void onNavigate?.({ kind: "board", boardSlug: board, view: "list", query: "focus=search" }); }} />
          <Button variant="default" size="sm" icon="plus" disabled={!board} onClick={() => { if (board) void onNavigate?.({ kind: "board", boardSlug: board, view: "list", query: "create=1" }); }}>新建任务</Button>
          <IconButton className="mobile-appearance" icon={resolvedTheme === 'dark' ? 'sun' : 'moon'} label="切换浅色与深色" onClick={() => preferences.setTheme(resolvedTheme === 'dark' ? 'light' : 'dark')} />
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
