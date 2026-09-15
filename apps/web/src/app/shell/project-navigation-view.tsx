import type { MouseEventHandler, ReactNode } from 'react';
import { Icon, type IconName } from '../../components/ui/icon';
import { IconButton } from '../../components/ui/button';
import { SidebarTooltip } from '../../components/ui/sidebar-tooltip';
import './project-navigation.css';
export type ProjectNavKey = 'tasks' | 'events' | 'runs' | 'settings';
export interface ProjectNavigationViewProps {
  collapsed: boolean; mobile?: boolean; projectName: string; actor: string;
  active: ProjectNavKey; hasProject: boolean; dark: boolean;
  onToggle: () => void; onProject: () => void; onSearch: () => void;
  onNavigate: (key: ProjectNavKey) => void; onTheme: () => void;
  taskHref: string; onTaskClick?: MouseEventHandler<HTMLAnchorElement>;
  projectDialog?: ReactNode;
}
const items: readonly {key:ProjectNavKey;label:string;icon:IconName}[] = [
  {key:'tasks',label:'任务',icon:'task'},
  {key:'events',label:'项目动态',icon:'pulse'},
  {key:'runs',label:'运行记录',icon:'play'},
];
function NavigationBrand(p: ProjectNavigationViewProps) {
  return <div className="pn-brand-row"><a className="pn-brand" href={p.taskHref} aria-label="返回任务" onClick={p.onTaskClick}><span className="pn-emblem"><Icon name="layers" size={22}/></span>{!p.collapsed&&<span>Kanban</span>}</a>
      {!p.mobile&&<SidebarTooltip enabled={p.collapsed} label={p.collapsed?'展开侧边栏':'收起侧边栏'}><IconButton icon={p.collapsed?'panelExpand':'panelCollapse'} label={p.collapsed?'展开侧边栏':'收起侧边栏'} aria-expanded={!p.collapsed} aria-controls="desktop-project-navigation" onClick={p.onToggle}/></SidebarTooltip>}
    </div>;
}
/** 只展示已接入的导航入口，路由和偏好操作由调用者提供。 */
export function ProjectNavigationView(p: ProjectNavigationViewProps) {
  return <div className={'project-navigation ui-navigation'+(p.collapsed?' navigation-compact':'')} data-collapsed={p.collapsed} data-testid="product-side-nav">
    <NavigationBrand {...p}/>
    <SidebarTooltip label={p.projectName} enabled={p.collapsed}><button type="button" className="pn-project" aria-label="选择项目" onClick={p.onProject}><span className="pn-project-icon">{p.projectName.slice(0,1).toUpperCase()}</span>{!p.collapsed&&<><span className="pn-project-text"><strong>{p.projectName}</strong><small>当前项目</small></span><Icon name="down" size={13}/></>}</button></SidebarTooltip>
    <SidebarTooltip label="搜索任务" enabled={p.collapsed}><button type="button" className="pn-search" aria-label="搜索任务" disabled={!p.hasProject} onClick={p.onSearch}><Icon name="search" size={16}/>{!p.collapsed&&<><span>搜索任务</span><kbd>⌘ K</kbd></>}</button></SidebarTooltip>
    <div className="pn-navigation-scroll"><nav className="pn-items" aria-label="项目页面">{items.map(item=><SidebarTooltip key={item.key} label={item.label} enabled={p.collapsed}><button type="button" className={'pn-item'+(p.active===item.key?' active':'')} onClick={()=>p.onNavigate(item.key)} disabled={!p.hasProject&&item.key!=='tasks'} aria-current={p.active===item.key?'page':undefined} aria-label={item.label} data-testid={'nav-'+(item.key==='tasks'?'board':item.key)}><Icon name={item.icon} size={17}/>{!p.collapsed&&<span>{item.label}</span>}</button></SidebarTooltip>)}</nav></div>
    <NavigationFooter {...p}/>{p.projectDialog}
  </div>;
}
function NavigationFooter(p: ProjectNavigationViewProps) {
  return <div className="pn-bottom"><SidebarTooltip label={p.dark?'切换浅色':'切换深色'} enabled={p.collapsed}><button className="pn-theme" type="button" role="switch" aria-label="深色主题" aria-checked={p.dark} onClick={p.onTheme}><Icon name={p.dark?'moon':'sun'} size={17}/>{!p.collapsed&&<><span>{p.dark?'深色模式':'浅色模式'}</span><span className="pn-switch" aria-hidden="true"><i/></span></>}</button></SidebarTooltip>
      <SidebarTooltip label="设置" enabled={p.collapsed}><button type="button" className={'pn-profile'+(p.active==='settings'?' active':'')} onClick={()=>p.onNavigate('settings')} aria-label="设置" aria-current={p.active==='settings'?'page':undefined} data-testid="nav-settings"><span className="pn-avatar">{p.actor.slice(0,1)||'K'}</span>{!p.collapsed&&<><span className="pn-profile-copy"><strong>{p.actor||'本地用户'}</strong><small>设置</small></span><Icon name="settings" size={16}/></>}</button></SidebarTooltip>
    </div>;
}
