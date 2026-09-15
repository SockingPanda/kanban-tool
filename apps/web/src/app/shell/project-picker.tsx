import { useId } from 'react';
import { useBoardDirectory } from '../../application/workspace/use-board-directory';
import { parseCanonicalBoardSlug, type CanonicalBoardSlug } from '../../domain/board-slug';
import type { AppNavigationTarget } from '../../application/navigation/router';
import { Select, type ChoiceOption } from '../../components/ui/select';
import { Button } from '../../components/ui/button';

export function ProjectPicker({ selected, onNavigate }: {selected?:CanonicalBoardSlug;onNavigate?:(target:AppNavigationTarget)=>void|Promise<unknown>}) {
  const id=useId(), boards=useBoardDirectory();
  const options:ChoiceOption[]=(boards.data??[]).map(board=>({value:board.slug,label:board.name,disabled:board.archived,description:board.archived?'已归档':board.slug}));
  if(selected&&!options.some(option=>option.value===selected))options.unshift({value:selected,label:selected});
  return <div className="project-picker">
    <label htmlFor={id}>当前项目</label>
    <Select id={id} aria-label="选择项目" value={selected??''} options={options} searchable placeholder={boards.loading?'正在加载项目…':'搜索或选择项目'} disabled={boards.loading&&!boards.data} onValueChange={value=>{
      const slug=parseCanonicalBoardSlug(value);if(slug)void onNavigate?.({kind:'board',boardSlug:slug,view:'list'});
    }}/>
    {boards.error&&<div role="alert">项目加载失败。<Button size="sm" onClick={boards.retry}>重试</Button></div>}
  </div>;
}
