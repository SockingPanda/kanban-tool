import { useResolvedTheme } from '../../platform/preferences/use-resolved-theme';
import { useState, type MouseEvent } from 'react';
import { Icon, type IconName } from '../../components/ui/icon';
import { IconButton } from '../../components/ui/button';
import { SidebarTooltip } from '../../components/ui/sidebar-tooltip';
import { Dialog } from '../../components/ui/dialog';
import { cn } from '../../components/ui/classes';
import { routePath, type AppNavigationTarget } from '../../application/navigation/router';
import { usePreferences } from '../../platform/preferences/use-preferences';
import type { ProductShellProps } from './shell-contract';
import { ProjectPicker } from './project-picker';
const projectPages: {key:string;label:string;icon:IconName;hint:string}[] = [
  {key:'map-pending',label:'项目地图',icon:'map',hint:'能力与依赖'}, {key:'board',label:'任务',icon:'task',hint:'推动具体变化'}, {key:'cycles',label:'迭代',icon:'cycle',hint:'安排工作时间'}, {key:'modules',label:'模块',icon:'box',hint:'组织交付范围'}, {key:'provenance',label:'代码追溯',icon:'branch',hint:'Git AI 来源'},
];
export function ProjectNavigation({ runtime, route, canonicalBoardSlug, onNavigate, mobile = false }: Pick<ProductShellProps,'runtime'|'route'|'canonicalBoardSlug'|'onNavigate'> & {mobile?:boolean}) {
  const preferences=usePreferences(),collapsed=!mobile&&!preferences.sidebarExpanded;
  const [projectsOpen,setProjectsOpen]=useState(false);
  const view=route.kind==='board'?route.view??'list':route.kind;
  const taskActive=['list','board','map'].includes(view);
  const taskTarget:AppNavigationTarget=canonicalBoardSlug?{kind:'board',boardSlug:canonicalBoardSlug,view:'list'}:{kind:'home'};
  const navigate=(event:MouseEvent<HTMLAnchorElement>,target:AppNavigationTarget)=>{if(!onNavigate||event.button!==0||event.ctrlKey||event.metaKey||event.altKey||event.shiftKey)return;event.preventDefault();void onNavigate(target);};
  const go=(target:AppNavigationTarget)=>{void onNavigate?.(target);};
  return <div className={cn('project-navigation',collapsed&&'navigation-compact')} data-collapsed={collapsed} data-testid="product-side-nav">
    <div className="sidebar-brand-row"><a className="sidebar-brand" href={routePath(taskTarget,{basePath:runtime.webBasePath})} onClick={event=>navigate(event,taskTarget)} aria-label="返回任务"><span className="sidebar-emblem"><Icon name="layers" size={23} /></span><span className="sidebar-wordmark">atlas<span>.</span></span></a>{!mobile&&<SidebarTooltip label={collapsed?'展开导航':'收起导航'} enabled={collapsed}><IconButton className="sidebar-collapse-button" icon={collapsed?'panelExpand':'panelCollapse'} label={collapsed?'展开侧边栏':'收起侧边栏'} aria-expanded={!collapsed} aria-controls="desktop-project-navigation" onClick={()=>preferences.setSidebarExpanded(collapsed)} /></SidebarTooltip>}</div>
    <button type="button" className="project-identity" title={canonicalBoardSlug??'选择项目'} aria-label="选择项目" onClick={()=>setProjectsOpen(true)}><span className="project-monogram">K</span><div><strong>{canonicalBoardSlug??'选择项目'}</strong><small>个人项目</small></div><span className="online-dot" /></button>
    <SidebarTooltip label="搜索项目 · Ctrl / ⌘ K" enabled={collapsed}><button type="button" className="sidebar-search" aria-label="搜索项目" onClick={()=>{if(canonicalBoardSlug)go({kind:'board',boardSlug:canonicalBoardSlug,view:'list',query:'focus=search'});}}><Icon name="search" size={15} /><span>搜索项目</span><kbd>⌘ K</kbd></button></SidebarTooltip>
    <div className="sidebar-scroll"><div className="nav-section-caption">工作区域 <span>PROJECT</span></div><nav className="main-nav" aria-label="项目页面">{projectPages.map((item,index)=><SidebarTooltip key={item.key} enabled={collapsed} label={`${item.label} · ${item.hint}`}><button type="button" className={cn('sidebar-nav-item',item.key==='board'&&taskActive&&'active')} onClick={()=>go(taskTarget)} disabled={item.key!=='board'} title={item.key!=='board'?'尚未接入':undefined} aria-current={item.key==='board'&&taskActive?'page':undefined} aria-label={item.key==='board'?item.label:`${item.label} · 尚未接入`} data-testid={`nav-${item.key}`}><span className="nav-item-index">{String(index+1).padStart(2,'0')}</span><Icon name={item.icon} size={17} /><span className="nav-label">{item.label}</span>{item.key!=='board'&&<small className="nav-count">尚未接入</small>}</button></SidebarTooltip>)}</nav>
    <div className="nav-section-caption pinned-caption">收藏模块 <span>PINNED</span></div><div className="sidebar-favorites" />
    <div className="sidebar-secondary">{([{key:'events',label:'项目动态',icon:'pulse'},{key:'runs',label:'运行记录',icon:'play'}] as const).map(item=><SidebarTooltip key={item.key} enabled={collapsed} label={item.label}><button className={view===item.key?'active':''} aria-label={item.label} data-testid={`nav-${item.key}`} disabled={!canonicalBoardSlug} onClick={()=>{if(canonicalBoardSlug)go({kind:'board',boardSlug:canonicalBoardSlug,view:item.key});}}><Icon name={item.icon} size={16} /><span>{item.label}</span></button></SidebarTooltip>)}<button type="button" aria-label="使用手册" disabled title="尚未接入"><Icon name="book" size={16} /><span>使用手册</span></button><button type="button" aria-label="组件预览" disabled title="尚未接入"><Icon name="grid" size={16} /><span>组件预览</span></button></div></div>
    <div className="sidebar-bottom"><div className="sidebar-edition"><span>FOLIO</span><small>纸本</small></div><NavigationAppearance collapsed={collapsed} /><SidebarTooltip label="设置" enabled={collapsed}><button className="sidebar-profile" onClick={()=>go({kind:'settings'})} aria-label="设置" aria-current={["settings","health","maintenance"].includes(route.kind)?"page":undefined} data-testid="nav-settings"><span className="avatar">{(preferences.actor||runtime.actor).slice(0,1)}</span><div><strong>{preferences.actor||runtime.actor}</strong><small>本地工作空间</small></div><Icon name="settings" size={16} /></button></SidebarTooltip></div>
    <Dialog open={projectsOpen} onClose={()=>setProjectsOpen(false)} title="选择项目"><ProjectPicker selected={canonicalBoardSlug} onNavigate={target=>{setProjectsOpen(false);return onNavigate?.(target);}} /></Dialog>
  </div>;
}

function NavigationAppearance({collapsed}:{collapsed:boolean}) {
  const preferences=usePreferences(),resolvedTheme=useResolvedTheme();
  return (<button type="button" role="switch" aria-checked={resolvedTheme==='dark'} aria-label="深色主题" onClick={()=>preferences.setTheme(resolvedTheme==='dark'?'light':'dark')} className={collapsed?'appearance-toggle appearance-compact':'appearance-toggle'}>{collapsed?<Icon name={resolvedTheme==='dark'?'moon':'sun'} size={18}/>:<><span className={resolvedTheme!=='dark'?'selected':''}><Icon name="sun" size={15}/>浅色</span><span className={resolvedTheme==='dark'?'selected':''}><Icon name="moon" size={15}/>深色</span></>}</button>);
}
