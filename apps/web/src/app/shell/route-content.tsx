import { useEffect, useState, type ReactNode } from 'react';
import { Button } from '../../components/ui/button';
import { usePreferences } from '../../platform/preferences/use-preferences';
import { createTranslator, type MessageKey } from '../../application/i18n';
import { BrowserConnectivityProvider } from '../../platform/connectivity/browser-connectivity-provider';
// 应用组合通过功能公共接口，避免依赖私有实现；最小复现见 build/react-doctor-regressions.test.ts。
// react-doctor-disable-next-line react-doctor/no-barrel-import
import { ExplorerPage } from '../../features/tasks/index';
import { HealthPage } from '../../features/health/index';
import { MaintenancePage } from '../../features/maintenance/index';
import { SettingsPage } from '../../features/settings/index';
import type { ProductShellProps, ShellBoundary } from './shell-contract';
import styles from '../../components/layout/boundary.module.css';

type Boundary = { id: string; title: MessageKey; description?: MessageKey; detail?: ReactNode; path?: string; alert?: boolean; retry?: boolean };
function routeBoundary(route: ProductShellProps['route'], boundary: ShellBoundary, error: ReactNode): Boundary | null {
  if (boundary === 'loading') return { id: 'shell-loading', title: 'loading' };
  if (boundary === 'error') return { id: 'shell-error', title: 'error', description: 'errorDescription', detail: error, alert: true, retry: true };
  if (boundary === 'offline' && !['board', 'health', 'maintenance', 'settings'].includes(route.kind)) return { id: 'shell-offline', title: 'offline', description: 'offlineDescription' };
  if (route.kind === 'home') return { id: 'shell-home-loading', title: 'homeLoading', description: 'homeLoadingDescription' };
  if (route.kind === 'not-found') return { id: 'shell-not-found', title: 'notFound', description: 'notFoundDescription', path: route.pathname, alert: true };
  if (route.kind === 'error') return { id: 'shell-route-error', title: 'invalidBoardSlug', description: 'invalidBoardSlugDescription', path: route.pathname, alert: true };
  return null;
}
function useOnline() {
  const [online, setOnline] = useState(() => typeof navigator === 'undefined' || navigator.onLine);
  useEffect(() => {
    const connected = () => setOnline(true), disconnected = () => setOnline(false);
    window.addEventListener('online', connected);
    window.addEventListener('offline', disconnected);
    return () => { window.removeEventListener('online', connected); window.removeEventListener('offline', disconnected); };
  }, []);
  return online;
}
function BoundaryPanel({ boundary, onRetry }: { boundary: Boundary; onRetry?: () => void }) {
  const { locale } = usePreferences();
  const t = createTranslator(locale);
  const description = boundary.detail ?? (boundary.description ? t(boundary.description) : undefined);
  return <section className={styles.boundary} role={boundary.alert ? 'alert' : 'status'} aria-live="polite" data-testid={boundary.id}>
    <p className={styles.eyebrow}>{t('routeBoundary')}</p><h1>{t(boundary.title)}</h1>
    {description && <p>{description}</p>}{boundary.path && <code translate="no">{boundary.path}</code>}
    {boundary.retry && onRetry && <Button label={t('retry')} variant="secondary" onClick={onRetry} />}
  </section>;
}
function CurrentRoute({ props, online }: { props: ProductShellProps; online: boolean }) {
  const { route, runtime, canonicalBoardSlug, onNavigate, onReconnect, taskMutations, onVisibleCanonicalReloadChange, syncStatus, eventsBatch, invalidationRevision = 0 } = props;
  if (route.kind === 'settings') return <SettingsPage runtime={runtime} boardSlug={canonicalBoardSlug} onNavigate={onNavigate} onReconnect={onReconnect} />;
  if (route.kind === 'health') return <HealthPage runtime={runtime} />;
  if (route.kind === 'maintenance') return <MaintenancePage runtime={runtime} boardSlug={route.boardSlug} />;
  if (route.kind !== 'board') return null;
  return <ExplorerPage runtime={runtime} route={route} onNavigate={onNavigate} online={online} invalidationRevision={invalidationRevision}
    boardRevision={props.boardRevision ?? invalidationRevision} inspectorRevision={props.inspectorRevision ?? invalidationRevision}
    runsRevision={props.runsRevision ?? invalidationRevision} eventsRefreshRevision={props.eventsRefreshRevision ?? invalidationRevision}
    eventsBatch={eventsBatch} syncStatus={syncStatus} taskMutations={taskMutations} onVisibleCanonicalReloadChange={onVisibleCanonicalReloadChange} />;
}
export function RouteContent(props: ProductShellProps) {
  const online = useOnline();
  if (props.route.kind === 'home' && props.children) return <BrowserConnectivityProvider online={online}>{props.children}</BrowserConnectivityProvider>;
  const boundary = routeBoundary(props.route, props.boundary ?? (online ? 'ready' : 'offline'), props.error);
  if (boundary) return <BoundaryPanel boundary={boundary} onRetry={props.onRetry} />;
  return <>{props.children && <div hidden aria-hidden="true" data-testid="board-live-session">{props.children}</div>}<CurrentRoute props={props} online={online} /></>;
}
