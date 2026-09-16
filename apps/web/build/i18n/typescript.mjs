import { createRequire } from 'node:module';
import path from 'node:path';
const require = createRequire(import.meta.url);
export function loadTypeScript() {
  const candidates = [process.env.KANBAN_TYPESCRIPT_PATH, 'typescript',
    path.join(process.cwd(), 'node_modules/typescript'),
    path.join(process.cwd(), 'apps/web/node_modules/typescript')].filter(Boolean);
  for (const candidate of candidates) {
    try { return require(candidate); } catch (error) {
      if (error.code !== 'MODULE_NOT_FOUND') throw error;
    }
  }
  throw new Error('TypeScript is required. Run pnpm install in the repository, or set KANBAN_TYPESCRIPT_PATH to an installed TypeScript package.');
}
export const ts = loadTypeScript();
