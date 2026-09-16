import { useState, type MouseEvent } from 'react';
import { useResolvedTheme } from '../../platform/preferences/use-resolved-theme';
import { usePreferences } from '../../platform/preferences/use-preferences';
import { useI18n } from '../../platform/localization/use-i18n';
import { Dialog } from '../../components/ui/dialog';
import { routePath, type AppNavigationTarget } from '../../application/navigation/router';
import type { ProductShellProps } from './shell-contract';
import { ProjectPicker } from './project-picker';
import { ProjectNavigationView, type ProjectNavKey } from './project-navigation-view';

function activeNavigation(route:ProductShellProps['route']):ProjectNavKey {
const view=route.kind==='board'?route.view??'list':route.kind;
const objectType=route.kind==='board'?new URLSearchParams(route.query).get('objects'):null;
return ['settings','health','maintenance'].includes(view)?'settings':objectType==='module'?'modules':objectType==='cycle'?'cycles':objectType?'objects':view==='events'?'events':view==='runs'?'runs':'tasks';
}
function navigationTarget(key:ProjectNavKey,boardSlug:ProductShellProps['canonicalBoardSlug']):AppNavigationTarget {
 if(key==='settings')return {kind:'settings'};
 if(!boardSlug)return {kind:'home'};
 if(key==='modules'||key==='cycles'||key==='objects')return {kind:'board',boardSlug,view:'list',query:'objects='+({modules:'module',cycles:'cycle',objects:'note'}[key])};
 return {kind:'board',boardSlug,view:key==='tasks'?'list':key};
}
function plainClick(event:MouseEvent<HTMLAnchorElement>){return !event.defaultPrevented&&event.button===0&&!event.ctrlKey&&!event.metaKey&&!event.altKey&&!event.shiftKey;}

export function ProjectNavigation({runtime,route,canonicalBoardSlug,onNavigate,mobile=false}:Pick<ProductShellProps,'runtime'|'route'|'canonicalBoardSlug'|'onNavigate'>&{mobile?:boolean}) {
  const preferences=usePreferences(),resolvedTheme=useResolvedTheme(),{t}=useI18n();
  const [projectsOpen,setProjectsOpen]=useState(false);
  const active=activeNavigation(route);
  const taskTarget:AppNavigationTarget=canonicalBoardSlug?{kind:'board',boardSlug:canonicalBoardSlug,view:'list'}:{kind:'home'};
  const navigateTask=(event:MouseEvent<HTMLAnchorElement>)=>{
    if(!onNavigate||!plainClick(event))return;
    event.preventDefault();void onNavigate(taskTarget);
  };
  const go=(target:AppNavigationTarget)=>{void onNavigate?.(target);};
  const navigate=(key:ProjectNavKey)=>go(navigationTarget(key,canonicalBoardSlug));
  return <ProjectNavigationView collapsed={!mobile&&!preferences.sidebarExpanded} mobile={mobile}
    taskHref={routePath(taskTarget,{basePath:runtime.webBasePath})} onTaskClick={navigateTask}
    projectName={canonicalBoardSlug??t('shell:chooseProject')}
    actor={preferences.actor||runtime.actor} active={active} hasProject={Boolean(canonicalBoardSlug)} dark={resolvedTheme==='dark'}
    onToggle={()=>preferences.setSidebarExpanded(!preferences.sidebarExpanded)} onProject={()=>setProjectsOpen(true)} onNavigate={navigate}
    onSearch={()=>{if(canonicalBoardSlug)go({kind:'board',boardSlug:canonicalBoardSlug,view:'list',query:'focus=search'});}}
    onTheme={()=>preferences.setTheme(resolvedTheme==='dark'?'light':'dark')}
    projectDialog={<Dialog open={projectsOpen} onClose={()=>setProjectsOpen(false)} title={t('shell:chooseProject')}><ProjectPicker selected={canonicalBoardSlug} onNavigate={target=>{setProjectsOpen(false);return onNavigate?.(target);}}/></Dialog>}
  />;
}
