import { useId, useState } from 'react';
import { Button } from '../../components/ui/button';
import { Input } from '../../components/ui/input';
import { Select } from '../../components/ui/select';
import { Icon } from '../../components/ui/icon';
import { SettingsSection, SettingsRow } from '../../components/ui/settings-section';
import './settings.css';

export type SettingsTab = 'appearance' | 'identity' | 'connection';
export interface SettingsViewProps {
  initialTab?: SettingsTab;
  hasProject?: boolean;
  theme: 'light' | 'dark' | 'system';
  density: 'comfortable' | 'compact';
  locale: 'zh' | 'en';
  sidebarExpanded: boolean;
  onThemeChange: (value: string) => void;
  onDensityChange: (value: string) => void;
  onLocaleChange: (value: string) => void;
  onSidebarChange: (value: boolean) => void;
  actorDraft: string;
  actorError: string | null;
  actorSaved: boolean;
  onActorChange: (value: string) => void;
  onActorSave: () => void;
  onActorReset: () => void;
  connection: 'loading' | 'available' | 'warning' | 'error';
  connectionMessage: string;
  healthErrorNextStep: string | null;
  healthPending: boolean;
  onHealthRetry: () => void;
  reconnectPending: boolean;
  reconnectEnabled: boolean;
  reconnectFeedback: string | null;
  onReconnect: () => void;
  onHealth?: () => void;
  onMaintenance?: () => void;
  onCopyDiagnostics: () => void;
  copyPending: boolean;
  copyFeedback: string | null;
  diagnosticFacts: readonly { label: string; value: string }[];
}

const copy = {
  zh: { title:'设置', description:'管理界面偏好、操作身份与本机服务。', appearance:'外观', identity:'操作身份', connection:'连接与诊断', display:'界面显示', displayNote:'修改立即应用，只影响当前浏览器。', theme:'主题', themeNote:'跟随系统，或固定使用浅色、深色。', light:'浅色', dark:'深色', system:'跟随系统', density:'信息密度', densityNote:'调整列表和控件的间距。', comfortable:'舒适', compact:'紧凑', sidebar:'左侧导航', sidebarNote:'收起后仍保留图标入口。', expanded:'展开', collapsed:'收起为图标栏', language:'语言', languageNote:'用于支持多语言的页面和控件。', identityNote:'这个名字会写入后续操作记录，不是登录账号。', actor:'操作名称', actorPlaceholder:'输入操作名称', save:'保存名称', reset:'使用服务默认值', saved:'操作名称已保存。', service:'本机服务', serviceNote:'状态来自最近一次健康检查；与实时事件同步分开判断。', available:'服务可用', warning:'需要检查', error:'服务不可用', loading:'正在检查', reconnect:'重新连接', reconnecting:'正在重连…', health:'系统状态', maintenance:'数据维护', tools:'诊断工具', toolsNote:'出现连接或数据问题时再查看。', technical:'技术详情', copy:'复制诊断信息', copying:'正在复制…' },
  en: { title:'Settings', description:'Appearance, operation identity and local service.', appearance:'Appearance', identity:'Identity', connection:'Connection', display:'Display', displayNote:'Changes apply immediately in this browser.', theme:'Theme', themeNote:'Follow the system, or choose light or dark.', light:'Light', dark:'Dark', system:'System', density:'Density', densityNote:'Adjust spacing in lists and controls.', comfortable:'Comfortable', compact:'Compact', sidebar:'Sidebar', sidebarNote:'The collapsed rail keeps navigation available.', expanded:'Expanded', collapsed:'Icon rail', language:'Language', languageNote:'Applied to localized pages and controls.', identityNote:'This name identifies future operations. It is not a login account.', actor:'Operation name', actorPlaceholder:'Enter a name', save:'Save name', reset:'Use service default', saved:'Operation name saved.', service:'Local service', serviceNote:'Based on the last health check, independent of event sync.', available:'Service available', warning:'Check required', error:'Service unavailable', loading:'Checking', reconnect:'Reconnect', reconnecting:'Reconnecting…', health:'System status', maintenance:'Data maintenance', tools:'Diagnostics', toolsNote:'Inspect these when troubleshooting.', technical:'Technical details', copy:'Copy diagnostics', copying:'Copying…' },
};

/** 只负责设置界面展示，异步操作由生产容器提供。 */
export function SettingsView(props: SettingsViewProps) {
  const [tab, setTab] = useState<SettingsTab>(props.initialTab ?? 'appearance');
  const id = useId(), c = copy[props.locale];
  const tabs: { value: SettingsTab; label: string; icon: 'sun' | 'user' | 'database' }[] = [
    { value:'appearance',label:c.appearance,icon:'sun' },
    { value:'identity',label:c.identity,icon:'user' },
    { value:'connection',label:c.connection,icon:'database' },
  ];
  return <div className="settings-page" data-testid="settings-page">
    <header className="settings-page-heading"><h1>{c.title}</h1><p>{c.description}</p></header>
    <div className="settings-tabs" role="tablist" aria-label={c.title}>
      {tabs.map((item,index)=><button key={item.value} type="button" role="tab" id={id+'-'+item.value} aria-selected={tab===item.value} aria-controls={id+'-panel-'+item.value} tabIndex={tab===item.value?0:-1} onClick={()=>setTab(item.value)} onKeyDown={event=>{
        if (!['ArrowLeft','ArrowRight','Home','End'].includes(event.key)) return;
        event.preventDefault();
        const next = event.key==='Home'?0:event.key==='End'?tabs.length-1:(index+(event.key==='ArrowRight'?1:-1)+tabs.length)%tabs.length;
        setTab(tabs[next].value);document.getElementById(id+'-'+tabs[next].value)?.focus();
      }}><Icon name={item.icon} size={16}/>{item.label}</button>)}
    </div>
    <div role="tabpanel" id={id+'-panel-'+tab} aria-labelledby={id+'-'+tab} tabIndex={0} className="settings-tab-panel">
      {tab==='appearance'&&<AppearancePanel settings={props} copy={c} id={id}/>}
      {tab==='identity'&&<IdentityPanel settings={props} copy={c} id={id}/>}
      {tab==='connection'&&<ConnectionPanel settings={props} copy={c} id={id}/>}
    </div>
  </div>;
}

type PanelProps = { settings: SettingsViewProps; copy: typeof copy.zh; id: string };

function AppearancePanel({settings:props,copy:c,id}:PanelProps) {
  return <SettingsSection title={c.display} description={c.displayNote}>
        <SettingsRow label={c.theme} description={c.themeNote} controlId={id+'-theme'}><Select id={id+'-theme'} aria-label={c.theme} data-testid="appearance-theme" value={props.theme} onValueChange={props.onThemeChange} options={[{value:'system',label:c.system,icon:'panel'},{value:'light',label:c.light,icon:'sun'},{value:'dark',label:c.dark,icon:'moon'}]}/></SettingsRow>
        <SettingsRow label={c.density} description={c.densityNote} controlId={id+'-density'}><Select id={id+'-density'} aria-label={c.density} data-testid="appearance-density" value={props.density} onValueChange={props.onDensityChange} options={[{value:'comfortable',label:c.comfortable},{value:'compact',label:c.compact}]}/></SettingsRow>
        <SettingsRow label={c.sidebar} description={c.sidebarNote} controlId={id+'-sidebar'}><Select id={id+'-sidebar'} aria-label={c.sidebar} value={props.sidebarExpanded?'expanded':'collapsed'} onValueChange={value=>props.onSidebarChange(value==='expanded')} options={[{value:'expanded',label:c.expanded},{value:'collapsed',label:c.collapsed}]}/></SettingsRow>
        <SettingsRow label={c.language} description={c.languageNote} controlId={id+'-locale'}><Select id={id+'-locale'} aria-label={c.language} data-testid="settings-locale" value={props.locale} onValueChange={props.onLocaleChange} options={[{value:'zh',label:'简体中文'},{value:'en',label:'English'}]}/></SettingsRow>
      </SettingsSection>;
}

function IdentityPanel({settings:props,copy:c,id}:PanelProps) {
  return <SettingsSection title={c.identity} description={c.identityNote}>
        <form className="settings-identity-form" onSubmit={event=>{event.preventDefault();props.onActorSave();}}>
          <label htmlFor={id+'-actor'}>{c.actor}</label>
          <Input id={id+'-actor'} name="actor" data-testid="identity-actor" value={props.actorDraft} placeholder={c.actorPlaceholder} onChange={event=>props.onActorChange(event.target.value)} aria-invalid={Boolean(props.actorError)} aria-describedby={props.actorError?id+'-actor-error':undefined}/>
          {props.actorError&&<p className="settings-feedback is-error" id={id+'-actor-error'} role="alert">{props.actorError}</p>}
          <div className="settings-button-row"><Button type="submit" variant="default" data-testid="identity-save">{c.save}</Button><Button onClick={props.onActorReset} data-testid="identity-reset">{c.reset}</Button></div>
          {props.actorSaved&&<p className="settings-feedback" role="status">{c.saved}</p>}
        </form>
      </SettingsSection>;
}

function ConnectionPanel({settings:props,copy:c}:PanelProps) {
  const healthRetryLabel = props.locale === 'en' ? 'Retry health check' : '重试健康检查';
  return <div className="settings-connection-stack">
        <SettingsSection title={c.service} description={c.serviceNote}>
          <div className="settings-connection-summary"><span className={'settings-service-status is-'+props.connection} role="status" data-testid="settings-health-status"><Icon name={props.connection==='available'?'checkCircle':props.connection==='loading'?'active':'warning'} size={18}/>{c[props.connection]}</span><Button onClick={props.onReconnect} disabled={!props.reconnectEnabled||props.reconnectPending} data-testid="connection-reconnect">{props.reconnectPending?c.reconnecting:c.reconnect}</Button></div>
          {props.healthErrorNextStep ? <div className="settings-health-error" role="alert" data-testid="settings-health-error">
            <p className="settings-service-message">{props.connectionMessage}</p>
            <p className="settings-feedback">{props.healthErrorNextStep}</p>
            <Button onClick={props.onHealthRetry} disabled={props.healthPending} data-testid="settings-health-retry">{props.healthPending?c.loading:healthRetryLabel}</Button>
          </div> : <p className="settings-service-message">{props.connectionMessage}</p>}
          {props.reconnectFeedback&&<p className="settings-feedback" role="status">{props.reconnectFeedback}</p>}
          <div className="settings-button-row service-links" data-testid={props.hasProject===false?'settings-no-board':undefined}><Button icon="shield" data-testid="diagnostics-health-link" onClick={props.onHealth} disabled={!props.onHealth}>{c.health}</Button><Button icon="database" onClick={props.onMaintenance} disabled={!props.onMaintenance}>{c.maintenance}</Button></div>
        </SettingsSection>
        <SettingsSection title={c.tools} description={c.toolsNote}>
          <div className="settings-diagnostics-actions"><Button icon="file" onClick={props.onCopyDiagnostics} disabled={props.copyPending} data-testid="diagnostics-copy">{props.copyPending?c.copying:c.copy}</Button>{props.copyFeedback&&<p className="settings-feedback" role="status">{props.copyFeedback}</p>}</div>
          <details className="settings-technical"><summary>{c.technical}</summary><dl>{props.diagnosticFacts.map(fact=><div key={fact.label}><dt>{fact.label}</dt><dd translate="no">{fact.value}</dd></div>)}</dl></details>
        </SettingsSection>
      </div>;
}
