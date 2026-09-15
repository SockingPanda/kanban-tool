import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, test, vi } from 'vitest';
import { TaskTable } from './task-table';
import type { TaskListRow } from './task-list-model';
const row: TaskListRow = { id:'t_first',ref:'default#1',title:'<script>任务</script>',status:'ready',priority:2,assignee:null,executionPlanState:'planned',dependencyBlocked:false,requiredStepCount:2,completedRequiredStepCount:1,optionalStepCount:0,updatedAt:1 };
describe('TaskTable',()=>{
  test('按真实状态分组，任务内容转义，行和选择框可访问',()=>{
    const markup=renderToStaticMarkup(<TaskTable rows={[row]} selected={['t_first']} onSelect={vi.fn()} onSelectTask={vi.fn()} />);
    expect(markup).toContain('<table');
    expect(markup).toContain('<tbody');
    expect(markup).toContain('scope="col"');
    expect(markup).toContain('aria-label="选择 default#1"');
    expect(markup).toContain('data-task-opener="t_first"');
    expect(markup).toContain('&lt;script&gt;任务&lt;/script&gt;');
    expect(markup).not.toContain('<script>');
    expect(markup).toContain('1/2');
    expect(markup).toContain('已就绪');
  });
  test('空查询展示明确的空状态',()=>{
    const markup=renderToStaticMarkup(<TaskTable rows={[]} onSelectTask={vi.fn()} />);
    expect(markup).toContain('没有匹配的任务');
    expect(markup).not.toContain('data-task-id');
  });
});
