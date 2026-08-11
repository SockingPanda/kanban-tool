import { attentionTasks, type AttentionLens } from "../attention/attention-lens"
import type { BoardColumnViewModel, BoardTaskViewModel, BoardViewModel } from "./types"

function orderedVisibleColumns(columns: readonly BoardColumnViewModel[]) {
  return columns
    .filter((column) => !column.hidden)
    .slice()
    .sort((left, right) => left.position - right.position)
}

function tasksForColumn(model: BoardViewModel, column: BoardColumnViewModel): readonly BoardTaskViewModel[] {
  return (model.tasksByStatus[column.status] ?? []).slice().sort((left, right) => left.position - right.position)
}

/**
 * Attention lenses narrow the cards rendered in a column, not the board's
 * mutation surface. Keep every visible server column in the result so its
 * section remains a legal drag/drop target even when it has no matching card.
 */
export function boardColumnsForAttention(model: BoardViewModel, lens: AttentionLens | null) {
  return orderedVisibleColumns(model.columns).map((column) => ({
    column,
    tasks: attentionTasks(tasksForColumn(model, column), lens),
  }))
}
