import { useBoardSession, type BoardLiveProps } from '../../application/workspace/use-board-session';
import { BoardView } from './BoardView';
export type { BoardLiveProps } from '../../application/workspace/use-board-session';
export function BoardLive(props: BoardLiveProps) {
  const state = useBoardSession(props);
  return props.renderBoard === false ? null : <BoardView {...state} id="paper-board" />;
}
