import { ProjectPicker } from './project-picker';
import type { MouseEvent } from 'react';
import { Icon, type IconName } from "../../components/ui/icon";
import { IconButton } from "../../components/ui/button";
import { cn } from "../../components/ui/classes";
import { routePath, type AppNavigationTarget } from "../../application/navigation/router";
import { usePreferences } from "../../platform/preferences/use-preferences";
import type { ProductShellProps } from "./shell-contract";

const pendingPages: { label: string; icon: IconName }[] = [
  { label: '模块', icon: 'layers' }, { label: '迭代', icon: 'cycle' },
  { label: '项目能力地图', icon: 'map' }, { label: 'Git AI 追溯', icon: 'branch' },
];

export function ProjectNavigation({ runtime, route, canonicalBoardSlug, onNavigate, mobile = false }: Pick<ProductShellProps, 'runtime' | 'route' | 'canonicalBoardSlug' | 'onNavigate'> & { mobile?: boolean }) {
  const preferences = usePreferences();
  const collapsed = !mobile && !preferences.sidebarExpanded;
  const active = route.kind === 'board' ? route.view ?? 'board' : route.kind;
  const items: { label: string; icon: IconName; key: string; target: AppNavigationTarget; disabled?: boolean }[] = [
    { label: '任务', icon: 'task', key: 'board', target: canonicalBoardSlug ? { kind: 'board', boardSlug: canonicalBoardSlug, view: 'list' } : { kind: 'home' } },
    ...canonicalBoardSlug ? [
      { label: '依赖图', icon: 'tree' as const, key: 'map', target: { kind: 'board' as const, boardSlug: canonicalBoardSlug, view: 'map' as const } },
      { label: '运行记录', icon: 'play' as const, key: 'runs', target: { kind: 'board' as const, boardSlug: canonicalBoardSlug, view: 'runs' as const } },
      { label: '项目动态', icon: 'pulse' as const, key: 'events', target: { kind: 'board' as const, boardSlug: canonicalBoardSlug, view: 'events' as const } },
      { label: '健康', icon: 'shield' as const, key: 'health', target: { kind: 'health' as const, boardSlug: canonicalBoardSlug } },
      { label: '维护', icon: 'database' as const, key: 'maintenance', target: { kind: 'maintenance' as const, boardSlug: canonicalBoardSlug } },
    ] : [],
    { label: '设置', icon: 'settings', key: 'settings', target: { kind: 'settings' } },
  ];
  function navigate(event: MouseEvent<HTMLAnchorElement>, target: AppNavigationTarget) {
    if (!onNavigate || event.button !== 0 || event.metaKey || event.ctrlKey || event.altKey || event.shiftKey) return;
    event.preventDefault();
    void Promise.resolve(onNavigate(target)).catch(() => undefined);
  }
  return <nav className={cn('project-navigation', collapsed && 'navigation-compact')} aria-label="项目导航" data-testid="product-side-nav">
    <div className="sidebar-brand-row">
      <a href={runtime.webBasePath} className="sidebar-brand" onClick={e => navigate(e, { kind: 'home' })} aria-label="Kanban Tool 首页">
        <span className="sidebar-emblem"><Icon name="grid" size={28} /></span><span className="sidebar-wordmark">Kanban<span>.</span></span>
      </a>
      {!mobile && <IconButton className="sidebar-collapse-button" icon={collapsed ? 'panelExpand' : 'panelCollapse'} label={collapsed ? '展开侧栏' : '收起侧栏'} aria-expanded={!collapsed} aria-controls="desktop-project-navigation" onClick={() => preferences.setSidebarExpanded(collapsed)} />}
    </div>
    <div className="project-identity"><span className="project-monogram">{canonicalBoardSlug?.slice(0, 1).toUpperCase() ?? 'K'}</span><div><strong>{canonicalBoardSlug ?? '选择项目'}</strong><small>个人工作空间</small></div></div>
    <ProjectPicker selected={canonicalBoardSlug} onNavigate={onNavigate} />
    <div className="sidebar-scroll">
      <div className="nav-section-caption">工作空间<span>WORKSPACE</span></div>
      <div className="main-nav">{items.map(item => <a key={item.key} href={typeof item.target === 'string' ? item.target : routePath(item.target, { basePath: runtime.webBasePath })} onClick={e => navigate(e, item.target)} className={cn('sidebar-nav-item', (active === item.key || item.key === 'board' && active === 'list') && 'active')} aria-current={active === item.key || item.key === 'board' && active === 'list' ? 'page' : undefined} aria-label={item.label} title={collapsed ? item.label : undefined} data-testid={`nav-${item.key}`}><Icon name={item.icon} /><span className="nav-label">{item.label}</span></a>)}</div>
      <div className="nav-section-caption">规划中<span>COMING LATER</span></div>
      <div className="main-nav">{pendingPages.map(item => <button key={item.label} type="button" className="sidebar-nav-item" disabled title={`${item.label} · 尚未接入`} aria-label={`${item.label} · 尚未接入`}><Icon name={item.icon} /><span className="nav-label">{item.label}</span><small className="nav-count">尚未接入</small></button>)}</div>
    </div>
    <div className="sidebar-bottom"><div className="sidebar-edition"><span>PAPER / 纸本</span><small>本机 · {runtime.serverVersion}</small></div>
      <div className="appearance-toggle" role="group" aria-label="外观">{(['light', 'dark', 'system'] as const).map((mode, index) => <button type="button" key={mode} aria-pressed={preferences.theme === mode} className={preferences.theme === mode ? 'selected' : ''} onClick={() => preferences.setTheme(mode)} title={['浅色', '深色', '跟随系统'][index]}><Icon name={(['sun', 'moon', 'panel'] as const)[index]} size={14} />{!collapsed && ['浅色', '深色', '系统'][index]}</button>)}</div>
    </div>
  </nav>;
}
