/** Board Events 只读查询 seam；canonical identity 与 transport 规则仍由 explorer-read-model 持有。 */
export {
  BOARD_EVENTS_PAGE_LIMIT,
  ExplorerReadError,
  buildBoardEventsRequest,
  loadBoardEvents,
  mergeBoardEvents,
  parseBoardEvent,
  type BoardEventsBatch,
  type BoardEventsReadModel,
  type ExplorerEvent,
} from "./explorer-read-model"
