import { useSyncExternalStore } from 'react';
import { usePreferences } from './use-preferences';
const read = () => matchMedia('(prefers-color-scheme: dark)').matches;
const server = () => false;
function subscribe(notify: () => void) {
  const media = matchMedia('(prefers-color-scheme: dark)');
  media.addEventListener('change', notify);
  return () => media.removeEventListener('change', notify);
}
export function useResolvedTheme() {
  const { theme } = usePreferences();
  const dark = useSyncExternalStore(subscribe, read, server);
  return theme === 'system' ? dark ? 'dark' : 'light' : theme;
}
