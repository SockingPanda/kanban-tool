import { useId } from 'react';
import { useBoardDirectory } from '../../application/workspace/use-board-directory';
import { parseCanonicalBoardSlug, type CanonicalBoardSlug } from '../../domain/board-slug';
import type { AppNavigationTarget } from '../../application/navigation/router';
export function ProjectPicker({ selected, onNavigate }: { selected?: CanonicalBoardSlug; onNavigate?: (target: AppNavigationTarget) => void | Promise<unknown> }) {
  const id = useId();
  const boards = useBoardDirectory();
  return <div className="project-picker">
    <label htmlFor={id}>当前项目</label>
    <select id={id} aria-label="选择项目" value={selected ?? ''} onChange={event => {
      const slug = parseCanonicalBoardSlug(event.target.value);
      if (slug) void onNavigate?.({ kind: 'board', boardSlug: slug, view: 'list' });
    }} disabled={boards.loading && !boards.data}>
      {!selected && <option value="">{boards.loading ? '正在加载项目…' : '选择项目'}</option>}
      {selected && !boards.data?.some(board => board.slug === selected) && <option value={selected}>{selected}</option>}
      {boards.data?.map(board => <option key={board.id} value={board.slug} disabled={board.archived}>{board.name}{board.archived ? ' · 已归档' : ''}</option>)}
    </select>
    {boards.error && <div role="alert">项目加载失败。<button type="button" onClick={boards.retry}>重试</button></div>}
  </div>;
}
