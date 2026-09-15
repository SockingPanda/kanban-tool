import { useBoardSession, type BoardLiveProps } from '../../application/workspace/use-board-session';
import { Button } from '../../components/ui/button';
export type { BoardLiveProps } from '../../application/workspace/use-board-session';
export function BoardLive(props: BoardLiveProps) {
  const { state, onRetry } = useBoardSession(props);
  if (props.renderBoard === false || state.kind === 'ready') return null;
  return <section className="paper-banner" role={state.kind === 'error' ? 'alert' : 'status'}><p>{state.kind === 'loading' ? '正在加载项目…' : state.kind === 'empty' ? state.detail : state.message}</p>{state.kind !== 'loading' && <Button onClick={onRetry}>重试</Button>}</section>;
}
