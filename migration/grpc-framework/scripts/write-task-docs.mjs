#!/usr/bin/env node
// tasks.json 是离线任务事实源；此工具只渲染文档，不调用真实看板。
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
const root=path.resolve(path.dirname(fileURLToPath(import.meta.url)),'..');
const plan=JSON.parse(fs.readFileSync(path.join(root,'tasks/tasks.json'),'utf8'));
let md=`# Atlas 后续迁移任务\n\n基线：${plan.baseBranch}，${plan.baseCommit}。\n\n[机器可读计划](../tasks/tasks.json) 不是 kanban import 格式，也未向真实看板创建任务。执行 agent 使用实际 CLI/MCP 创建任务后另存 Gxx 到 t_... 的映射。\n\n## 执行关系\n\nG01 联编先行。G02 装配完成后可以立即进行 G04 的 Atlas gRPC 刷新接线；它不必等待完整分页 live query。G03 契约、G05 CLI/MCP 和 G07 完整投影随后覆盖各业务，G06 审查写路径与测量开销，G08 汇总真实联调，G09 才移除旧通道。精确依赖以以下各任务及 JSON 为准。\n\n代码、测试、文档和证据应随每个任务一次交付；综合评审在 G08/G09 进行。当前所有任务状态为 todo，框架的离线通过不等同于完成这些集成任务。\n`;
for(const t of plan.tasks){
 md+=`\n## ${t.id} ${t.title}\n\nOwner：${t.owner}。依赖：${t.dependencies.join('、')||'无'}。状态：${t.status}。\n\n输入：${t.inputs.map(x=>'`'+x+'`').join('、')}。\n\n### 实施\n\n`+t.steps.map((s,i)=>`${i+1}. ${s}`).join('\n')+'\n\n### 验收\n\n'+t.acceptance.map(s=>'- '+s).join('\n')+'\n\n交付：'+t.outputs.join('；')+'。\n\n非目标：'+t.nonGoals.join('；')+'。\n';
}
fs.writeFileSync(path.join(root,'docs/06-tasks.md'),md);
