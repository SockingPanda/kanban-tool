import type { MouseEventHandler, ReactNode } from 'react';
import { Icon, type IconName } from '../../components/ui/icon';
import { IconButton } from '../../components/ui/button';
import { SidebarTooltip } from '../../components/ui/sidebar-tooltip';
import { useI18n } from '../../platform/localization/use-i18n';
import type { PlainMessageKey } from '../../platform/localization/contracts.generated';
import './project-navigation.css';
export type ProjectNavKey='tasks'|'modules'|'cycles'|'objects'|'events'|'runs'|'settings';
export interface ProjectNavigationViewProps {
  collapsed:boolean;mobile?:boolean;projectName:string;actor:string;active:ProjectNavKey;hasProject:boolean;dark:boolean;
  onToggle:()=>void;onProject:()=>void;onSearch:()=>void;onNavigate:(key:ProjectNavKey)=>void;onTheme:()=>void;
  taskHref:string;onTaskClick?:MouseEventHandler<HTMLAnchorElement>;projectDialog?:ReactNode;
}
const items:readonly{key:ProjectNavKey;label:PlainMessageKey;icon:IconName}[]=[
 {key:'tasks',label:'shell:page.tasks',icon:'task'},
 {key:'modules',label:'shell:page.modules',icon:'layers'},
 {key:'cycles',label:'shell:page.cycles',icon:'play'},
 {key:'objects',label:'shell:page.objects',icon:'layers'},
 {key:'events',label:'shell:page.events',icon:'pulse'},
 {key:'runs',label:'shell:page.runs',icon:'play'},
];
function NavigationBrand(p:ProjectNavigationViewProps){
 const {t}=useI18n(),toggle=t(p.collapsed?'shell:expandSidebar':'shell:collapseSidebar');
 return <div className="pn-brand-row"><a className="pn-brand" href={p.taskHref} aria-label={t('shell:backToTasks')} onClick={p.onTaskClick}><span className="pn-emblem"><Icon name="layers" size={22}/></span>{!p.collapsed&&<span translate="no">Kanban</span>}</a>
  {!p.mobile&&<SidebarTooltip enabled={p.collapsed} label={toggle}><IconButton icon={p.collapsed?'panelExpand':'panelCollapse'} label={toggle} aria-expanded={!p.collapsed} aria-controls="desktop-project-navigation" onClick={p.onToggle}/></SidebarTooltip>}
 </div>;
}
export function ProjectNavigationView(p:ProjectNavigationViewProps){
 const {t}=useI18n();
 return <div className={'project-navigation ui-navigation'+(p.collapsed?' navigation-compact':'')} data-collapsed={p.collapsed} data-testid="product-side-nav">
  <NavigationBrand {...p}/>
  <SidebarTooltip label={p.projectName} enabled={p.collapsed}><button type="button" className="pn-project" aria-label={t('shell:chooseProject')} onClick={p.onProject}><span className="pn-project-icon">{p.projectName.slice(0,1).toUpperCase()}</span>{!p.collapsed&&<><span className="pn-project-text"><strong>{p.projectName}</strong><small>{t('shell:currentProject')}</small></span><Icon name="down" size={13}/></>}</button></SidebarTooltip>
  <SidebarTooltip label={t('shell:searchTasks')} enabled={p.collapsed}><button type="button" className="pn-search" aria-label={t('shell:searchTasks')} disabled={!p.hasProject} onClick={p.onSearch}><Icon name="search" size={16}/>{!p.collapsed&&<><span>{t('shell:searchTasks')}</span><kbd translate="no">⌘ K</kbd></>}</button></SidebarTooltip>
  <div className="pn-navigation-scroll"><nav className="pn-items" aria-label={t('shell:projectPages')}>{items.map(item=><SidebarTooltip key={item.key} label={t(item.label)} enabled={p.collapsed}><button type="button" className={'pn-item'+(p.active===item.key?' active':'')} onClick={()=>p.onNavigate(item.key)} disabled={!p.hasProject&&item.key!=='tasks'} aria-current={p.active===item.key?'page':undefined} aria-label={t(item.label)} data-testid={'nav-'+(item.key==='tasks'?'board':item.key)}><Icon name={item.icon} size={17}/>{!p.collapsed&&<span>{t(item.label)}</span>}</button></SidebarTooltip>)}</nav></div>
  <NavigationFooter {...p}/>{p.projectDialog}
 </div>;
}
function NavigationFooter(p:ProjectNavigationViewProps){
 const {t}=useI18n();
 return <div className="pn-bottom"><SidebarTooltip label={t(p.dark?'shell:toLight':'shell:toDark')} enabled={p.collapsed}><button className="pn-theme" type="button" role="switch" aria-label={t('shell:darkThemeLabel')} aria-checked={p.dark} onClick={p.onTheme}><Icon name={p.dark?'moon':'sun'} size={17}/>{!p.collapsed&&<><span>{t(p.dark?'shell:darkMode':'shell:lightMode')}</span><span className="pn-switch" aria-hidden="true"><i/></span></>}</button></SidebarTooltip>
  <SidebarTooltip label={t('common:settings')} enabled={p.collapsed}><button type="button" className={'pn-profile'+(p.active==='settings'?' active':'')} onClick={()=>p.onNavigate('settings')} aria-label={t('common:settings')} aria-current={p.active==='settings'?'page':undefined} data-testid="nav-settings"><span className="pn-avatar" translate="no">{p.actor.slice(0,1)||'K'}</span>{!p.collapsed&&<><span className="pn-profile-copy"><strong>{p.actor||t('shell:localUser')}</strong><small>{t('common:settings')}</small></span><Icon name="settings" size={16}/></>}</button></SidebarTooltip>
 </div>;
}
