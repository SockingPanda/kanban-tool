import { renderToStaticMarkup } from '../../../tests/support/render';
import { describe, expect, test, vi } from 'vitest';
import { PreferencesProvider } from '../../platform/preferences/preferences-provider';
import { assertCanonicalBoardSlug } from '../../domain/board-slug';
import type { WebRuntimeConfig } from '../../lib/runtime';
import type { HealthReport } from '../../application/data/health-read-model';
import { apiOriginForRuntime, diagnosticsText } from '../../application/diagnostics';
import { SettingsPage, type SettingsPageProps } from './SettingsPage';
const runtime={apiBaseUrl:'',webBasePath:'/app/',actor:'local',defaultBoard:'default',serverVersion:'3.0.0',protocolVersion:'v2',webBuildId:'build-test'} satisfies WebRuntimeConfig;
const health={ok:true,db:'turso',version:'3.0.0',db_path:'/tmp/kanban.db',db_fingerprint:'sha256:test'} satisfies HealthReport;
const render=(props:Partial<SettingsPageProps>={})=>renderToStaticMarkup(<PreferencesProvider><SettingsPage runtime={runtime} initialHealth={health} {...props}/></PreferencesProvider>);
describe('SettingsPage progressive disclosure',()=>{
  test('shows common appearance controls, not all settings at once',()=>{
    const html=render({boardSlug:assertCanonicalBoardSlug('default')});
    expect(html).toContain('data-testid="settings-page"');
    expect(html).toContain('data-testid="appearance-theme"');
    expect(html).toContain('data-testid="appearance-density"');
    expect(html).toContain('data-testid="settings-locale"');
    expect(html.match(/role="combobox"/g)).toHaveLength(4);
    expect(html).not.toContain('<select');
    expect(html).not.toContain('sha256:test');
    expect(html).not.toContain('data-testid="identity-actor"');
  });
  test('identity tab retains the name form and reset action',()=>{
    const html=render({initialTab:'identity'});
    expect(html).toContain('data-testid="identity-actor"');
    expect(html).toContain('data-testid="identity-save"');
    expect(html).toContain('data-testid="identity-reset"');
    expect(html).not.toContain('sha256:test');
  });
  test('connection diagnostics keep technical values in a closed disclosure',()=>{
    const html=render({initialTab:'connection',boardSlug:assertCanonicalBoardSlug('default'),onNavigate:vi.fn()});
    expect(html).toContain('data-testid="connection-reconnect"');
    expect(html).toContain('data-testid="diagnostics-copy"');
    expect(html).toContain('<details class="settings-technical">');
    expect(html).not.toMatch(/<details[^>]*\sopen/);
    expect(html).toContain('sha256:test');
  });
  test.each(['','selector:active'])('never treats runtime selector %s as a canonical board',defaultBoard=>{
    const html=render({initialTab:'connection',runtime:{...runtime,defaultBoard}});
    expect(html).toContain('data-testid="settings-no-board"');
    expect(html).toMatch(/<button[^>]*(?:disabled[^>]*data-testid="diagnostics-health-link"|data-testid="diagnostics-health-link"[^>]*disabled)/);
  });
  test('diagnostic copy still excludes server-controlled paths',()=>{
    expect(apiOriginForRuntime(runtime,'https://kanban.test/app/settings')).toBe('https://kanban.test');
    expect(diagnosticsText(runtime,health,'https://kanban.test/app/settings')).not.toContain('/tmp/kanban.db');
  });
  test('accepts the existing clipboard and reconnect seam',()=>{
    const html=render({initialTab:'connection',boardSlug:assertCanonicalBoardSlug('default'),clipboardWrite:vi.fn(async()=>undefined),onReconnect:vi.fn(()=>true)});
    expect(html).toContain('data-testid="connection-reconnect"');
    expect(html).toContain('data-testid="diagnostics-copy"');
  });
});
