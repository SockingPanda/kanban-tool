import type { CanonicalBoardSlug } from './board-slug';
export interface BoardOption { readonly id: string; readonly slug: CanonicalBoardSlug; readonly name: string; readonly archived: boolean }
