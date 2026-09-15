import { useRef, useState, type FormEvent } from 'react';
import type { TaskWorkspaceState } from './use-task-workspace';
import { Button } from '../../components/ui/button';
import { Textarea } from '../../components/ui/input';
import { Select } from '../../components/ui/select';
import { commentPageState, formatCommentDateTime, type CommentSortOrder } from './TaskInspectorRelationsPanel.logic';
import type { TaskInspectorViewModel } from './inspector-model';

function Comment({ comment }: { comment: TaskInspectorViewModel['comments'][number] }) {
  const date = formatCommentDateTime(comment.createdAt, 'zh');
  return <article className="comment">
    <div className="comment-meta"><span className="avatar tiny">{comment.author.slice(0, 1)}</span><strong>{comment.author}</strong><time dateTime={date.iso}>{date.label}</time></div>
    <p>{comment.body}</p>
    {comment.metadata && <details><summary>附加信息</summary><pre>{JSON.stringify(comment.metadata, null, 2)}</pre></details>}
  </article>;
}

export function TaskDiscussion({ workspace }: { workspace: TaskWorkspaceState }) {
  const [comment, setComment] = useState('');
  const [kind, setKind] = useState<'note' | 'decision' | 'signal'>('note');
  const [sort, setSort] = useState<CommentSortOrder>('oldest');
  const [page, setPage] = useState(0);
  const pending = useRef(false), handlers = workspace.inspectorMutationHandlers;
  const busy = !handlers || Boolean(workspace.inspectorMutationSnapshot?.pending.size);
  const submit = async (event: FormEvent) => {
    event.preventDefault();
    if (!comment.trim() || busy || pending.current) return;
    const submitted = comment;
    pending.current = true;
    try {
      const outcome = await handlers.addComment({ body: comment.trim(), kind });
      if (outcome.committed) setComment(current => current === submitted ? '' : current);
    } finally { pending.current = false; }
  };
  const comments = workspace.inspectorModel?.comments ?? [];
  const pagination = commentPageState(comments, page, 10, sort);
  return <div className="comments-panel" data-testid="task-discussion">
    {comments.length === 0 && <p className="muted">还没有讨论，记录一个决定或当前阻塞。</p>}
    {comments.length > 1 && <Select aria-label="讨论排序" value={sort} options={[{ value: 'oldest', label: '最早在前' }, { value: 'newest', label: '最新在前' }]} onValueChange={value => { setSort(value as CommentSortOrder); setPage(0); }} />}
    {pagination.comments.map(item => <Comment key={item.id} comment={item} />)}
    {pagination.pageCount > 1 && <div className="paper-pagination"><Button size="sm" disabled={!pagination.hasPreviousPage} onClick={() => setPage(pagination.page - 1)}>上一页讨论</Button><span>{pagination.page + 1} / {pagination.pageCount}</span><Button size="sm" disabled={!pagination.hasNextPage} onClick={() => setPage(pagination.page + 1)}>下一页讨论</Button></div>}
    <form onSubmit={event => { void submit(event); }}>
      <Textarea value={comment} name="comment-body" aria-label="评论内容" placeholder="记录讨论、决定或阻塞原因…" rows={4} onChange={event => setComment(event.target.value)} />
      <Select aria-label="评论类型" value={kind} options={[{ value: 'note', label: '笔记' }, { value: 'decision', label: '决定' }, { value: 'signal', label: '信号' }]} onValueChange={value => setKind(value as typeof kind)} />
      <Button type="submit" variant="default" disabled={!comment.trim() || busy}>{busy ? '保存中…' : '发布评论'}</Button>
    </form>
  </div>;
}
