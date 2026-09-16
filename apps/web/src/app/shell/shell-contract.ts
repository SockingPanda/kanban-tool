import type { ReactNode } from 'react';
import type { CanonicalBoardSlug } from "../../domain/board-slug";
import type { BoardTaskCanonicalReloadHandler, BoardTaskMutationSurface } from "../../application/tasks/task-mutation-state";
import type { BoardSyncStatus } from "../../domain/tasks/board";
import type { WebRuntimeConfig } from "../../lib/runtime";
import type { AppNavigationTarget, AppRoute } from "../../application/navigation/router";
import type { BoardReconnectResult } from "../../application/workspace/board-session-registry";
export type ShellBoundary = "ready" | "loading" | "error" | "offline"

export type ProductShellProps = {
  runtime: WebRuntimeConfig
  route: AppRoute
  canonicalBoardSlug?: CanonicalBoardSlug
  children?: ReactNode
  boundary?: ShellBoundary
  error?: ReactNode
  onNavigate?: (target: AppNavigationTarget) => void | Promise<unknown>
  onReconnect?: () => BoardReconnectResult | boolean | void | Promise<BoardReconnectResult | boolean | void>
  onRetry?: () => void
  syncStatus?: BoardSyncStatus
  taskMutations?: BoardTaskMutationSurface
  /** Register the currently visible Inspector reads for awaited canonical reloads. */
  onVisibleCanonicalReloadChange?: (reload: BoardTaskCanonicalReloadHandler | undefined, releasedReload?: BoardTaskCanonicalReloadHandler) => void
}
