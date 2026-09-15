import { useState, type MouseEvent } from 'react';
import { useResolvedTheme } from '../../platform/preferences/use-resolved-theme';
import { usePreferences } from '../../platform/preferences/use-preferences';
import { Dialog } from '../../components/ui/dialog';
import { routePath, type AppNavigationTarget } from '../../application/navigation/router';
import type { ProductShellProps } from './shell-contract';
import { ProjectPicker } from './project-picker';
import { ProjectNavigationView, type ProjectNavKey } from './project-navigation-view';

export function ProjectNavigation({ runtime, route, canonicalBoardSlug, onNavigate, mobile = false }: Pick<ProductShellProps, 'runtime'|'route'|'canonicalBoardSlug'|'onNavigate'> & {mobile?:boolean}) {
  const preferences=usePreferences(), resolvedTheme=useResolvedTheme();
  const [projectsOpen,setProjectsOpen]=useState(false);
  const view=route.kind==='board'?route.view??'list':route.kind;
  const active:ProjectNavKey=['settings','health','maintenance'].includes(view)?'settings':view==='events'?'events':view==='runs'?'runs':'tasks';
  const taskTarget:AppNavigationTarget=canonicalBoardSlug?{kind:'board',boardSlug:canonicalBoardSlug,view:'list'}:{kind:'home'};
  const navigateTask=(event:MouseEvent<HTMLAnchorElement>)=>{
    if(!onNavigate||event.defaultPrevented||event.button!==0||event.ctrlKey||event.metaKey||event.altKey||event.shiftKey)return;
    event.preventDefault();void onNavigate(taskTarget);
  };
  const go=(target:AppNavigationTarget)=>{void onNavigate?.(target);};
  const navigate=(key:ProjectNavKey)=>{
    if(key==='settings'){go({kind:'settings'});return;}
    if(!canonicalBoardSlug){go({kind:'home'});return;}
    go({kind:'board',boardSlug:canonicalBoardSlug,view:key==='tasks'?'list':key});
  };
  return <ProjectNavigationView collapsed={!mobile&&!preferences.sidebarExpanded} mobile={mobile}
    taskHref={routePath(taskTarget,{basePath:runtime.webBasePath})} onTaskClick={navigateTask}
    projectName={canonicalBoardSlug??'选择项目'}
    actor={preferences.actor||runtime.actor} active={active} hasProject={Boolean(canonicalBoardSlug)} dark={resolvedTheme==='dark'}
    onToggle={()=>preferences.setSidebarExpanded(!preferences.sidebarExpanded)}
    onProject={()=>setProjectsOpen(true)} onNavigate={navigate}
    onSearch={()=>{if(canonicalBoardSlug)go({kind:'board',boardSlug:canonicalBoardSlug,view:'list',query:'focus=search'});}}
    onTheme={()=>preferences.setTheme(resolvedTheme==='dark'?'light':'dark')}
    projectDialog={<Dialog open={projectsOpen} onClose={()=>setProjectsOpen(false)} title="选择项目"><ProjectPicker selected={canonicalBoardSlug} onNavigate={target=>{setProjectsOpen(false);return onNavigate?.(target);}}/></Dialog>}
  />;
}
