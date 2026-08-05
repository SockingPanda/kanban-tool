export * from "./api/types"
import { ApiTransport } from "./api/transport"
import { ApiError, loadRuntimeConfig } from "./api/transport"
import * as boards from "./api/boards"
import * as attachments from "./api/attachments"
import * as comments from "./api/comments"
import * as events from "./api/events"
import * as health from "./api/health"
import * as labels from "./api/labels"
import * as maintenance from "./api/maintenance"
import * as ontology from "./api/ontology"
import * as runs from "./api/runs"
import * as signals from "./api/signals"
import * as steps from "./api/steps"
import * as tasks from "./api/tasks"
import * as transitions from "./api/transitions"
import type { DesktopLocale } from "@/i18n"
import type {
  BoardListOptions,
  ClaimResponse,
  CreateAttachmentInput,
  CreateBoardInput,
  CreateStepInput,
  CreateTaskInput,
  LabelOntologyActionCreateInput,
  LabelOntologyReviewOptions,
  LabelOntologySignalListOptions,
  RequestOptions,
  RuntimeConfig,
  SearchTaskOptions,
  SignalListOptions,
  Task,
  TaskListOptions,
  UpdateStepInput,
  DownloadedAttachment,
} from "./api/types"

export { ApiError, loadRuntimeConfig }

export class KanbanApi {
  private readonly transport: ApiTransport

  constructor(config: RuntimeConfig, options: { locale?: DesktopLocale } = {}) {
    this.transport = new ApiTransport(config, options)
  }

  get actor() {
    return this.transport.actor
  }

  get board() {
    return this.transport.board
  }

  health(options: RequestOptions = {}) { return health.health(this.transport, options) }
  listBoards(options: BoardListOptions = {}) { return boards.listBoards(this.transport, options) }
  createBoard(input: CreateBoardInput, options: RequestOptions = {}) { return boards.createBoard(this.transport, input, options) }
  getBoard(board: string, options: RequestOptions = {}) { return boards.getBoard(this.transport, board, options) }
  archiveBoard(board: string, options: RequestOptions = {}) { return boards.archiveBoard(this.transport, board, options) }
  listAttachments(taskId: string, options: RequestOptions = {}) { return attachments.listAttachments(this.transport, taskId, options) }
  createAttachment(taskId: string, input: CreateAttachmentInput, options: RequestOptions = {}) { return attachments.createAttachment(this.transport, taskId, input, options) }
  downloadAttachment(taskId: string, attachmentId: string, options: RequestOptions = {}): Promise<DownloadedAttachment> { return attachments.downloadAttachment(this.transport, taskId, attachmentId, options) }
  deleteAttachment(taskId: string, attachmentId: string, options: RequestOptions = {}) { return attachments.deleteAttachment(this.transport, taskId, attachmentId, options) }
  stats(options: RequestOptions = {}) { return health.stats(this.transport, options) }
  searchStatus(options: RequestOptions = {}) { return health.searchStatus(this.transport, options) }
  doctor(options: RequestOptions = {}) { return maintenance.doctor(this.transport, options) }
  checkpoint(options: RequestOptions = {}) { return maintenance.checkpoint(this.transport, options) }
  listBoardColumns(options: RequestOptions = {}) { return boards.listBoardColumns(this.transport, options) }
  listTasks(options: TaskListOptions = {}) { return tasks.listTasks(this.transport, options) }
  listTasksByStatus(options: TaskListOptions & { statuses: import("./api/types").TaskStatus[] }) { return tasks.listTasksByStatus(this.transport, options) }
  searchTasks(options: SearchTaskOptions) { return tasks.searchTasks(this.transport, options) }
  searchTasksByStatus(options: SearchTaskOptions & { statuses: import("./api/types").TaskStatus[] }) { return tasks.searchTasksByStatus(this.transport, options) }
  createTask(input: CreateTaskInput, options: RequestOptions = {}) { return tasks.createTask(this.transport, input, options) }
  updateTask(taskId: string, patch: Partial<Pick<Task, "title" | "description" | "assignee" | "priority" | "due_at" | "scheduled_at">>, options: RequestOptions = {}) { return tasks.updateTask(this.transport, taskId, patch, options) }
  getTask(taskId: string, options: RequestOptions = {}) { return tasks.getTask(this.transport, taskId, options) }
  listDependencies(taskId: string, options: RequestOptions = {}) { return tasks.listDependencies(this.transport, taskId, options) }
  addDependency(taskId: string, parentTaskId: string, options: RequestOptions = {}) { return tasks.addDependency(this.transport, taskId, parentTaskId, options) }
  removeDependency(taskId: string, parentTaskId: string, options: RequestOptions = {}) { return tasks.removeDependency(this.transport, taskId, parentTaskId, options) }
  getTaskNeighborhood(taskId: string, options: RequestOptions & { depth?: number; limitNodes?: number } = {}) { return tasks.getTaskNeighborhood(this.transport, taskId, options) }
  getBoardTaskMap(board = this.board, options: RequestOptions & { activeOnly?: boolean; contextDepth?: number; includeDoneContext?: boolean; includeArchivedContext?: boolean; hideIsolated?: boolean; limitNodes?: number } = {}) { return tasks.getBoardTaskMap(this.transport, board, options) }
  listSteps(taskId: string, options: RequestOptions = {}) { return steps.listSteps(this.transport, taskId, options) }
  createStep(taskId: string, input: CreateStepInput, options: RequestOptions = {}) { return steps.createStep(this.transport, taskId, input, options) }
  updateStep(taskId: string, stepId: string, input: UpdateStepInput, options: RequestOptions = {}) { return steps.updateStep(this.transport, taskId, stepId, input, options) }
  removeStep(taskId: string, stepId: string, options: RequestOptions = {}) { return steps.removeStep(this.transport, taskId, stepId, options) }
  completeStep(taskId: string, stepId: string, note: string, options: RequestOptions = {}) { return steps.completeStep(this.transport, taskId, stepId, note, options) }
  skipStep(taskId: string, stepId: string, reason: string, options: RequestOptions = {}) { return steps.skipStep(this.transport, taskId, stepId, reason, options) }
  reopenStep(taskId: string, stepId: string, reason: string, options: RequestOptions = {}) { return steps.reopenStep(this.transport, taskId, stepId, reason, options) }
  markExecutionPlanNotRequired(taskId: string, reason: string, options: RequestOptions = {}) { return tasks.markExecutionPlanNotRequired(this.transport, taskId, reason, options) }
  listRuns(taskId: string, options: RequestOptions = {}) { return runs.listRuns(this.transport, taskId, options) }
  getRun(runId: string, options: RequestOptions = {}) { return runs.getRun(this.transport, runId, options) }
  getRunLog(runId: string, options: RequestOptions = {}) { return runs.getRunLog(this.transport, runId, options) }
  listComments(taskId: string, options: RequestOptions = {}) { return comments.listComments(this.transport, taskId, options) }
  createComment(taskId: string, body: string, options: RequestOptions = {}) { return comments.createComment(this.transport, taskId, body, options) }
  addTaskLabel(taskId: string, name: string, options: RequestOptions = {}) { return labels.addTaskLabel(this.transport, taskId, name, options) }
  suggestTaskLabels(taskId: string, options: RequestOptions & { limit?: number; candidateLimit?: number; atomLimit?: number; maxSelectedLabels?: number; minScore?: number } = {}) { return labels.suggestTaskLabels(this.transport, taskId, options) }
  removeTaskLabel(taskId: string, labelId: string, options: RequestOptions = {}) { return labels.removeTaskLabel(this.transport, taskId, labelId, options) }
  listSignals(options: SignalListOptions = {}) { return signals.listSignals(this.transport, options) }
  reviewSignals(options: SignalListOptions = {}) { return signals.reviewSignals(this.transport, options) }
  getSignal(signalId: string, options: RequestOptions = {}) { return signals.getSignal(this.transport, signalId, options) }
  listLabelOntologySignals(options: LabelOntologySignalListOptions = {}) { return ontology.listLabelOntologySignals(this.transport, options) }
  reviewLabelOntology(options: LabelOntologyReviewOptions = {}) { return ontology.reviewLabelOntology(this.transport, options) }
  getLabelOntologySignal(signalId: string, options: RequestOptions = {}) { return ontology.getLabelOntologySignal(this.transport, signalId, options) }
  async createLabelOntologyAction(input: LabelOntologyActionCreateInput, options: RequestOptions = {}) { return ontology.createLabelOntologyAction(this.transport, input, options) }
  explainLabelAtom(atomRef: string, options: RequestOptions = {}) { return ontology.explainLabelAtom(this.transport, atomRef, options) }
  listEvents(taskId: string, options: RequestOptions = {}) { return events.listEvents(this.transport, taskId, options) }
  listBoardEvents(options: { after?: number; limit?: number; signal?: AbortSignal } = {}) { return events.listBoardEvents(this.transport, options) }
  listEventsAfter(after: number, options: RequestOptions = {}) { return events.listEventsAfter(this.transport, after, options) }
  releaseTask(task: Task, claimToken: string, options: RequestOptions = {}): Promise<Task> { return transitions.releaseTask(this.transport, task, claimToken, options) }

  transition(task: Task, action: "specify" | "promote" | "heartbeat" | "release" | "submit-review" | "complete" | "block" | "reopen" | "unblock" | "archive", body?: Record<string, unknown>, options?: RequestOptions): Promise<Task>
  transition(task: Task, action: "claim", body?: Record<string, unknown>, options?: RequestOptions): Promise<Task | ClaimResponse>
  transition(task: Task, action: "specify" | "promote" | "claim" | "heartbeat" | "release" | "complete" | "reopen" | "submit-review" | "block" | "unblock" | "archive", body?: Record<string, unknown>, options?: RequestOptions): Promise<Task | ClaimResponse>
  transition(task: Task, action: "specify" | "promote" | "claim" | "heartbeat" | "release" | "complete" | "reopen" | "submit-review" | "block" | "unblock" | "archive", body: Record<string, unknown> = {}, options: RequestOptions = {}) {
    return transitions.transition(this.transport, task, action, body, options)
  }
}
