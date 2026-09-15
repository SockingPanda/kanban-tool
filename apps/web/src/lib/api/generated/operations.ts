// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
export const operations = [
  {
    "id": "api.health",
    "service": "kanban.v1.KanbanService",
    "method": "GetHealth",
    "path": "/kanban.v1.KanbanService/GetHealth",
    "request": "GetHealthRequest",
    "response": "GetHealthResponse",
    "serverStreaming": false,
    "obligations": {
      "path": {
        "kind": "not_applicable"
      },
      "query": {
        "kind": "not_applicable"
      },
      "headers": {
        "kind": "contract",
        "contractId": "api.health.headers"
      },
      "body": {
        "kind": "not_applicable"
      },
      "success": {
        "kind": "contract",
        "contractId": "api.health.response"
      }
    },
    "sharedComponents": []
  },
  {
    "id": "api.list-boards",
    "service": "kanban.v1.KanbanService",
    "method": "ListBoards",
    "path": "/kanban.v1.KanbanService/ListBoards",
    "request": "ListBoardsRequest",
    "response": "ListBoardsResponse",
    "serverStreaming": false,
    "obligations": {
      "path": {
        "kind": "not_applicable"
      },
      "query": {
        "kind": "contract",
        "contractId": "api.list-boards.query"
      },
      "headers": {
        "kind": "contract",
        "contractId": "api.list-boards.headers"
      },
      "body": {
        "kind": "not_applicable"
      },
      "success": {
        "kind": "contract",
        "contractId": "api.list-boards.response"
      }
    },
    "sharedComponents": []
  },
  {
    "id": "api.list-board-columns",
    "service": "kanban.v1.KanbanService",
    "method": "ListBoardColumns",
    "path": "/kanban.v1.KanbanService/ListBoardColumns",
    "request": "ListBoardColumnsRequest",
    "response": "ListBoardColumnsResponse",
    "serverStreaming": false,
    "obligations": {
      "path": {
        "kind": "contract",
        "contractId": "api.list-board-columns.path"
      },
      "query": {
        "kind": "not_applicable"
      },
      "headers": {
        "kind": "contract",
        "contractId": "api.list-board-columns.headers"
      },
      "body": {
        "kind": "not_applicable"
      },
      "success": {
        "kind": "contract",
        "contractId": "api.list-board-columns.response"
      }
    },
    "sharedComponents": []
  },
  {
    "id": "api.explain-label-atom",
    "service": "kanban.v1.KanbanService",
    "method": "ExplainLabelAtom",
    "path": "/kanban.v1.KanbanService/ExplainLabelAtom",
    "request": "ExplainLabelAtomRequest",
    "response": "ExplainLabelAtomResponse",
    "serverStreaming": false,
    "obligations": {
      "path": {
        "kind": "contract",
        "contractId": "api.label-atom.path"
      },
      "query": {
        "kind": "not_applicable"
      },
      "headers": {
        "kind": "contract",
        "contractId": "api.explain-label-atom.headers"
      },
      "body": {
        "kind": "not_applicable"
      },
      "success": {
        "kind": "contract",
        "contractId": "api.explain-label-atom.response"
      }
    },
    "sharedComponents": []
  },
  {
    "id": "api.list-tasks",
    "service": "kanban.v1.KanbanService",
    "method": "ListTasks",
    "path": "/kanban.v1.KanbanService/ListTasks",
    "request": "ListTasksRequest",
    "response": "ListTasksResponse",
    "serverStreaming": false,
    "obligations": {
      "path": {
        "kind": "contract",
        "contractId": "api.list-tasks.path"
      },
      "query": {
        "kind": "contract",
        "contractId": "api.list-tasks.query"
      },
      "headers": {
        "kind": "contract",
        "contractId": "api.list-tasks.headers"
      },
      "body": {
        "kind": "not_applicable"
      },
      "success": {
        "kind": "contract",
        "contractId": "api.list-tasks.response"
      }
    },
    "sharedComponents": [
      "api.error.response"
    ]
  },
  {
    "id": "api.list-tasks-by-status",
    "service": "kanban.v1.KanbanService",
    "method": "ListTasksByStatus",
    "path": "/kanban.v1.KanbanService/ListTasksByStatus",
    "request": "ListTasksByStatusRequest",
    "response": "ListTasksByStatusResponse",
    "serverStreaming": false,
    "obligations": {
      "path": {
        "kind": "contract",
        "contractId": "api.list-tasks-by-status.path"
      },
      "query": {
        "kind": "contract",
        "contractId": "api.list-tasks-by-status.query"
      },
      "headers": {
        "kind": "contract",
        "contractId": "api.list-tasks-by-status.headers"
      },
      "body": {
        "kind": "not_applicable"
      },
      "success": {
        "kind": "contract",
        "contractId": "api.list-tasks-by-status.response"
      }
    },
    "sharedComponents": [
      "api.error.response"
    ]
  },
  {
    "id": "api.create-task",
    "service": "kanban.v1.KanbanService",
    "method": "CreateTask",
    "path": "/kanban.v1.KanbanService/CreateTask",
    "request": "CreateTaskRequest",
    "response": "CreateTaskResponse",
    "serverStreaming": false,
    "obligations": {
      "path": {
        "kind": "contract",
        "contractId": "api.create-task.path"
      },
      "query": {
        "kind": "not_applicable"
      },
      "headers": {
        "kind": "contract",
        "contractId": "api.create-task.headers"
      },
      "body": {
        "kind": "contract",
        "contractId": "api.create-task.request"
      },
      "success": {
        "kind": "contract",
        "contractId": "api.create-task.response"
      }
    },
    "sharedComponents": []
  },
  {
    "id": "api.review-signals",
    "service": "kanban.v1.KanbanService",
    "method": "ReviewSignals",
    "path": "/kanban.v1.KanbanService/ReviewSignals",
    "request": "ReviewSignalsRequest",
    "response": "ReviewSignalsResponse",
    "serverStreaming": false,
    "obligations": {
      "path": {
        "kind": "contract",
        "contractId": "api.review-signals.path"
      },
      "query": {
        "kind": "contract",
        "contractId": "api.review-signals.query"
      },
      "headers": {
        "kind": "contract",
        "contractId": "api.review-signals.headers"
      },
      "body": {
        "kind": "not_applicable"
      },
      "success": {
        "kind": "contract",
        "contractId": "api.review-signals.response"
      }
    },
    "sharedComponents": []
  },
  {
    "id": "api.get-signal",
    "service": "kanban.v1.KanbanService",
    "method": "GetSignal",
    "path": "/kanban.v1.KanbanService/GetSignal",
    "request": "GetSignalRequest",
    "response": "GetSignalResponse",
    "serverStreaming": false,
    "obligations": {
      "path": {
        "kind": "contract",
        "contractId": "api.get-signal.path"
      },
      "query": {
        "kind": "not_applicable"
      },
      "headers": {
        "kind": "contract",
        "contractId": "api.get-signal.headers"
      },
      "body": {
        "kind": "not_applicable"
      },
      "success": {
        "kind": "contract",
        "contractId": "api.get-signal.response"
      }
    },
    "sharedComponents": []
  },
  {
    "id": "api.board-task-map",
    "service": "kanban.v1.KanbanService",
    "method": "BoardTaskMap",
    "path": "/kanban.v1.KanbanService/BoardTaskMap",
    "request": "BoardTaskMapRequest",
    "response": "BoardTaskMapResponse",
    "serverStreaming": false,
    "obligations": {
      "path": {
        "kind": "contract",
        "contractId": "api.board-task-map.path"
      },
      "query": {
        "kind": "contract",
        "contractId": "api.board-task-map.query"
      },
      "headers": {
        "kind": "contract",
        "contractId": "api.board-task-map.headers"
      },
      "body": {
        "kind": "not_applicable"
      },
      "success": {
        "kind": "contract",
        "contractId": "api.board-task-map.response"
      }
    },
    "sharedComponents": []
  },
  {
    "id": "api.get-task",
    "service": "kanban.v1.KanbanService",
    "method": "GetTask",
    "path": "/kanban.v1.KanbanService/GetTask",
    "request": "GetTaskRequest",
    "response": "GetTaskResponse",
    "serverStreaming": false,
    "obligations": {
      "path": {
        "kind": "contract",
        "contractId": "api.get-task.path"
      },
      "query": {
        "kind": "contract",
        "contractId": "api.get-task.query"
      },
      "headers": {
        "kind": "contract",
        "contractId": "api.get-task.headers"
      },
      "body": {
        "kind": "not_applicable"
      },
      "success": {
        "kind": "contract",
        "contractId": "api.get-task.response"
      }
    },
    "sharedComponents": []
  },
  {
    "id": "api.update-task",
    "service": "kanban.v1.KanbanService",
    "method": "UpdateTask",
    "path": "/kanban.v1.KanbanService/UpdateTask",
    "request": "UpdateTaskRequest",
    "response": "UpdateTaskResponse",
    "serverStreaming": false,
    "obligations": {
      "path": {
        "kind": "contract",
        "contractId": "api.update-task.path"
      },
      "query": {
        "kind": "not_applicable"
      },
      "headers": {
        "kind": "contract",
        "contractId": "api.update-task.headers"
      },
      "body": {
        "kind": "contract",
        "contractId": "api.update-task.request"
      },
      "success": {
        "kind": "contract",
        "contractId": "api.update-task.response"
      }
    },
    "sharedComponents": []
  },
  {
    "id": "api.task-neighborhood",
    "service": "kanban.v1.KanbanService",
    "method": "TaskNeighborhood",
    "path": "/kanban.v1.KanbanService/TaskNeighborhood",
    "request": "TaskNeighborhoodRequest",
    "response": "TaskNeighborhoodResponse",
    "serverStreaming": false,
    "obligations": {
      "path": {
        "kind": "contract",
        "contractId": "api.task-neighborhood.path"
      },
      "query": {
        "kind": "contract",
        "contractId": "api.task-neighborhood.query"
      },
      "headers": {
        "kind": "contract",
        "contractId": "api.task-neighborhood.headers"
      },
      "body": {
        "kind": "not_applicable"
      },
      "success": {
        "kind": "contract",
        "contractId": "api.task-neighborhood.response"
      }
    },
    "sharedComponents": []
  },
  {
    "id": "api.list-task-labels",
    "service": "kanban.v1.KanbanService",
    "method": "ListTaskLabels",
    "path": "/kanban.v1.KanbanService/ListTaskLabels",
    "request": "ListTaskLabelsRequest",
    "response": "ListTaskLabelsResponse",
    "serverStreaming": false,
    "obligations": {
      "path": {
        "kind": "contract",
        "contractId": "api.list-task-labels.path"
      },
      "query": {
        "kind": "not_applicable"
      },
      "headers": {
        "kind": "contract",
        "contractId": "api.list-task-labels.headers"
      },
      "body": {
        "kind": "not_applicable"
      },
      "success": {
        "kind": "contract",
        "contractId": "api.list-task-labels.response"
      }
    },
    "sharedComponents": [
      "api.error.response"
    ]
  },
  {
    "id": "api.add-task-label",
    "service": "kanban.v1.KanbanService",
    "method": "AddTaskLabel",
    "path": "/kanban.v1.KanbanService/AddTaskLabel",
    "request": "AddTaskLabelRequest",
    "response": "AddTaskLabelResponse",
    "serverStreaming": false,
    "obligations": {
      "path": {
        "kind": "contract",
        "contractId": "api.add-task-label.path"
      },
      "query": {
        "kind": "not_applicable"
      },
      "headers": {
        "kind": "contract",
        "contractId": "api.add-task-label.headers"
      },
      "body": {
        "kind": "contract",
        "contractId": "api.add-task-label.request"
      },
      "success": {
        "kind": "contract",
        "contractId": "api.add-task-label.response"
      }
    },
    "sharedComponents": [
      "api.error.response"
    ]
  },
  {
    "id": "api.suggest-task-labels",
    "service": "kanban.v1.KanbanService",
    "method": "SuggestTaskLabels",
    "path": "/kanban.v1.KanbanService/SuggestTaskLabels",
    "request": "SuggestTaskLabelsRequest",
    "response": "SuggestTaskLabelsResponse",
    "serverStreaming": false,
    "obligations": {
      "path": {
        "kind": "contract",
        "contractId": "api.suggest-task-labels.path"
      },
      "query": {
        "kind": "contract",
        "contractId": "api.label-suggestion.query"
      },
      "headers": {
        "kind": "contract",
        "contractId": "api.suggest-task-labels.headers"
      },
      "body": {
        "kind": "not_applicable"
      },
      "success": {
        "kind": "contract",
        "contractId": "api.suggest-task-labels.response"
      }
    },
    "sharedComponents": []
  },
  {
    "id": "api.list-label-ontology-signals",
    "service": "kanban.v1.KanbanService",
    "method": "ListLabelOntologySignals",
    "path": "/kanban.v1.KanbanService/ListLabelOntologySignals",
    "request": "ListLabelOntologySignalsRequest",
    "response": "ListLabelOntologySignalsResponse",
    "serverStreaming": false,
    "obligations": {
      "path": {
        "kind": "contract",
        "contractId": "api.list-label-ontology-signals.path"
      },
      "query": {
        "kind": "contract",
        "contractId": "api.label-ontology-signal.query"
      },
      "headers": {
        "kind": "contract",
        "contractId": "api.list-label-ontology-signals.headers"
      },
      "body": {
        "kind": "not_applicable"
      },
      "success": {
        "kind": "contract",
        "contractId": "api.list-label-ontology-signals.response"
      }
    },
    "sharedComponents": []
  },
  {
    "id": "api.review-label-ontology",
    "service": "kanban.v1.KanbanService",
    "method": "ReviewLabelOntology",
    "path": "/kanban.v1.KanbanService/ReviewLabelOntology",
    "request": "ReviewLabelOntologyRequest",
    "response": "ReviewLabelOntologyResponse",
    "serverStreaming": false,
    "obligations": {
      "path": {
        "kind": "contract",
        "contractId": "api.review-label-ontology.path"
      },
      "query": {
        "kind": "contract",
        "contractId": "api.label-ontology-review.query"
      },
      "headers": {
        "kind": "contract",
        "contractId": "api.review-label-ontology.headers"
      },
      "body": {
        "kind": "not_applicable"
      },
      "success": {
        "kind": "contract",
        "contractId": "api.review-label-ontology.response"
      }
    },
    "sharedComponents": []
  },
  {
    "id": "api.create-label-ontology-action",
    "service": "kanban.v1.KanbanService",
    "method": "CreateLabelOntologyAction",
    "path": "/kanban.v1.KanbanService/CreateLabelOntologyAction",
    "request": "CreateLabelOntologyActionRequest",
    "response": "CreateLabelOntologyActionResponse",
    "serverStreaming": false,
    "obligations": {
      "path": {
        "kind": "contract",
        "contractId": "api.create-label-ontology-action.path"
      },
      "query": {
        "kind": "not_applicable"
      },
      "headers": {
        "kind": "contract",
        "contractId": "api.create-label-ontology-action.headers"
      },
      "body": {
        "kind": "contract",
        "contractId": "api.create-label-ontology-action.request"
      },
      "success": {
        "kind": "contract",
        "contractId": "api.create-label-ontology-action.response"
      }
    },
    "sharedComponents": []
  },
  {
    "id": "api.get-label-ontology-signal",
    "service": "kanban.v1.KanbanService",
    "method": "GetLabelOntologySignal",
    "path": "/kanban.v1.KanbanService/GetLabelOntologySignal",
    "request": "GetLabelOntologySignalRequest",
    "response": "GetLabelOntologySignalResponse",
    "serverStreaming": false,
    "obligations": {
      "path": {
        "kind": "contract",
        "contractId": "api.get-label-ontology-signal.path"
      },
      "query": {
        "kind": "not_applicable"
      },
      "headers": {
        "kind": "contract",
        "contractId": "api.get-label-ontology-signal.headers"
      },
      "body": {
        "kind": "not_applicable"
      },
      "success": {
        "kind": "contract",
        "contractId": "api.get-label-ontology-signal.response"
      }
    },
    "sharedComponents": []
  },
  {
    "id": "api.remove-task-label",
    "service": "kanban.v1.KanbanService",
    "method": "RemoveTaskLabel",
    "path": "/kanban.v1.KanbanService/RemoveTaskLabel",
    "request": "RemoveTaskLabelRequest",
    "response": "RemoveTaskLabelResponse",
    "serverStreaming": false,
    "obligations": {
      "path": {
        "kind": "contract",
        "contractId": "api.remove-task-label.path"
      },
      "query": {
        "kind": "not_applicable"
      },
      "headers": {
        "kind": "contract",
        "contractId": "api.remove-task-label.headers"
      },
      "body": {
        "kind": "not_applicable"
      },
      "success": {
        "kind": "contract",
        "contractId": "api.remove-task-label.response"
      }
    },
    "sharedComponents": [
      "api.error.response"
    ]
  },
  {
    "id": "api.specify-task",
    "service": "kanban.v1.KanbanService",
    "method": "SpecifyTask",
    "path": "/kanban.v1.KanbanService/SpecifyTask",
    "request": "SpecifyTaskRequest",
    "response": "SpecifyTaskResponse",
    "serverStreaming": false,
    "obligations": {
      "path": {
        "kind": "contract",
        "contractId": "api.specify-task.path"
      },
      "query": {
        "kind": "not_applicable"
      },
      "headers": {
        "kind": "contract",
        "contractId": "api.specify-task.headers"
      },
      "body": {
        "kind": "contract",
        "contractId": "api.specify-task.request"
      },
      "success": {
        "kind": "contract",
        "contractId": "api.specify-task.response"
      }
    },
    "sharedComponents": []
  },
  {
    "id": "api.promote-task",
    "service": "kanban.v1.KanbanService",
    "method": "PromoteTask",
    "path": "/kanban.v1.KanbanService/PromoteTask",
    "request": "PromoteTaskRequest",
    "response": "PromoteTaskResponse",
    "serverStreaming": false,
    "obligations": {
      "path": {
        "kind": "contract",
        "contractId": "api.promote-task.path"
      },
      "query": {
        "kind": "not_applicable"
      },
      "headers": {
        "kind": "contract",
        "contractId": "api.promote-task.headers"
      },
      "body": {
        "kind": "contract",
        "contractId": "api.promote-task.request"
      },
      "success": {
        "kind": "contract",
        "contractId": "api.promote-task.response"
      }
    },
    "sharedComponents": []
  },
  {
    "id": "api.claim-task",
    "service": "kanban.v1.KanbanService",
    "method": "ClaimTask",
    "path": "/kanban.v1.KanbanService/ClaimTask",
    "request": "ClaimTaskRequest",
    "response": "ClaimTaskResponse",
    "serverStreaming": false,
    "obligations": {
      "path": {
        "kind": "contract",
        "contractId": "api.claim-task.path"
      },
      "query": {
        "kind": "not_applicable"
      },
      "headers": {
        "kind": "contract",
        "contractId": "api.claim-task.headers"
      },
      "body": {
        "kind": "contract",
        "contractId": "api.claim-task.request"
      },
      "success": {
        "kind": "contract",
        "contractId": "api.claim-task.response"
      }
    },
    "sharedComponents": []
  },
  {
    "id": "api.heartbeat-task",
    "service": "kanban.v1.KanbanService",
    "method": "HeartbeatTask",
    "path": "/kanban.v1.KanbanService/HeartbeatTask",
    "request": "HeartbeatTaskRequest",
    "response": "HeartbeatTaskResponse",
    "serverStreaming": false,
    "obligations": {
      "path": {
        "kind": "contract",
        "contractId": "api.heartbeat-task.path"
      },
      "query": {
        "kind": "not_applicable"
      },
      "headers": {
        "kind": "contract",
        "contractId": "api.heartbeat-task.headers"
      },
      "body": {
        "kind": "contract",
        "contractId": "api.heartbeat-task.request"
      },
      "success": {
        "kind": "contract",
        "contractId": "api.heartbeat-task.response"
      }
    },
    "sharedComponents": []
  },
  {
    "id": "api.complete-task",
    "service": "kanban.v1.KanbanService",
    "method": "CompleteTask",
    "path": "/kanban.v1.KanbanService/CompleteTask",
    "request": "CompleteTaskRequest",
    "response": "CompleteTaskResponse",
    "serverStreaming": false,
    "obligations": {
      "path": {
        "kind": "contract",
        "contractId": "api.complete-task.path"
      },
      "query": {
        "kind": "not_applicable"
      },
      "headers": {
        "kind": "contract",
        "contractId": "api.complete-task.headers"
      },
      "body": {
        "kind": "contract",
        "contractId": "api.complete-task.request"
      },
      "success": {
        "kind": "contract",
        "contractId": "api.complete-task.response"
      }
    },
    "sharedComponents": []
  },
  {
    "id": "api.submit-review-task",
    "service": "kanban.v1.KanbanService",
    "method": "SubmitReviewTask",
    "path": "/kanban.v1.KanbanService/SubmitReviewTask",
    "request": "SubmitReviewTaskRequest",
    "response": "SubmitReviewTaskResponse",
    "serverStreaming": false,
    "obligations": {
      "path": {
        "kind": "contract",
        "contractId": "api.submit-review-task.path"
      },
      "query": {
        "kind": "not_applicable"
      },
      "headers": {
        "kind": "contract",
        "contractId": "api.submit-review-task.headers"
      },
      "body": {
        "kind": "contract",
        "contractId": "api.submit-review-task.request"
      },
      "success": {
        "kind": "contract",
        "contractId": "api.submit-review-task.response"
      }
    },
    "sharedComponents": []
  },
  {
    "id": "api.block-task",
    "service": "kanban.v1.KanbanService",
    "method": "BlockTask",
    "path": "/kanban.v1.KanbanService/BlockTask",
    "request": "BlockTaskRequest",
    "response": "BlockTaskResponse",
    "serverStreaming": false,
    "obligations": {
      "path": {
        "kind": "contract",
        "contractId": "api.block-task.path"
      },
      "query": {
        "kind": "not_applicable"
      },
      "headers": {
        "kind": "contract",
        "contractId": "api.block-task.headers"
      },
      "body": {
        "kind": "contract",
        "contractId": "api.block-task.request"
      },
      "success": {
        "kind": "contract",
        "contractId": "api.block-task.response"
      }
    },
    "sharedComponents": []
  },
  {
    "id": "api.unblock-task",
    "service": "kanban.v1.KanbanService",
    "method": "UnblockTask",
    "path": "/kanban.v1.KanbanService/UnblockTask",
    "request": "UnblockTaskRequest",
    "response": "UnblockTaskResponse",
    "serverStreaming": false,
    "obligations": {
      "path": {
        "kind": "contract",
        "contractId": "api.unblock-task.path"
      },
      "query": {
        "kind": "not_applicable"
      },
      "headers": {
        "kind": "contract",
        "contractId": "api.unblock-task.headers"
      },
      "body": {
        "kind": "contract",
        "contractId": "api.unblock-task.request"
      },
      "success": {
        "kind": "contract",
        "contractId": "api.unblock-task.response"
      }
    },
    "sharedComponents": []
  },
  {
    "id": "api.archive-task",
    "service": "kanban.v1.KanbanService",
    "method": "ArchiveTask",
    "path": "/kanban.v1.KanbanService/ArchiveTask",
    "request": "ArchiveTaskRequest",
    "response": "ArchiveTaskResponse",
    "serverStreaming": false,
    "obligations": {
      "path": {
        "kind": "contract",
        "contractId": "api.archive-task.path"
      },
      "query": {
        "kind": "not_applicable"
      },
      "headers": {
        "kind": "contract",
        "contractId": "api.archive-task.headers"
      },
      "body": {
        "kind": "contract",
        "contractId": "api.archive-task.request"
      },
      "success": {
        "kind": "contract",
        "contractId": "api.archive-task.response"
      }
    },
    "sharedComponents": []
  },
  {
    "id": "api.list-dependencies",
    "service": "kanban.v1.KanbanService",
    "method": "ListDependencies",
    "path": "/kanban.v1.KanbanService/ListDependencies",
    "request": "ListDependenciesRequest",
    "response": "ListDependenciesResponse",
    "serverStreaming": false,
    "obligations": {
      "path": {
        "kind": "contract",
        "contractId": "api.list-dependencies.path"
      },
      "query": {
        "kind": "not_applicable"
      },
      "headers": {
        "kind": "contract",
        "contractId": "api.list-dependencies.headers"
      },
      "body": {
        "kind": "not_applicable"
      },
      "success": {
        "kind": "contract",
        "contractId": "api.list-dependencies.response"
      }
    },
    "sharedComponents": []
  },
  {
    "id": "api.add-dependency",
    "service": "kanban.v1.KanbanService",
    "method": "AddDependency",
    "path": "/kanban.v1.KanbanService/AddDependency",
    "request": "AddDependencyRequest",
    "response": "AddDependencyResponse",
    "serverStreaming": false,
    "obligations": {
      "path": {
        "kind": "contract",
        "contractId": "api.add-dependency.path"
      },
      "query": {
        "kind": "not_applicable"
      },
      "headers": {
        "kind": "contract",
        "contractId": "api.add-dependency.headers"
      },
      "body": {
        "kind": "contract",
        "contractId": "api.add-dependency.request"
      },
      "success": {
        "kind": "contract",
        "contractId": "api.add-dependency.response"
      }
    },
    "sharedComponents": []
  },
  {
    "id": "api.remove-dependency",
    "service": "kanban.v1.KanbanService",
    "method": "RemoveDependency",
    "path": "/kanban.v1.KanbanService/RemoveDependency",
    "request": "RemoveDependencyRequest",
    "response": "RemoveDependencyResponse",
    "serverStreaming": false,
    "obligations": {
      "path": {
        "kind": "contract",
        "contractId": "api.remove-dependency.path"
      },
      "query": {
        "kind": "not_applicable"
      },
      "headers": {
        "kind": "contract",
        "contractId": "api.remove-dependency.headers"
      },
      "body": {
        "kind": "not_applicable"
      },
      "success": {
        "kind": "contract",
        "contractId": "api.remove-dependency.response"
      }
    },
    "sharedComponents": []
  },
  {
    "id": "api.list-steps",
    "service": "kanban.v1.KanbanService",
    "method": "ListSteps",
    "path": "/kanban.v1.KanbanService/ListSteps",
    "request": "ListStepsRequest",
    "response": "ListStepsResponse",
    "serverStreaming": false,
    "obligations": {
      "path": {
        "kind": "contract",
        "contractId": "api.list-steps.path"
      },
      "query": {
        "kind": "not_applicable"
      },
      "headers": {
        "kind": "contract",
        "contractId": "api.list-steps.headers"
      },
      "body": {
        "kind": "not_applicable"
      },
      "success": {
        "kind": "contract",
        "contractId": "api.list-steps.response"
      }
    },
    "sharedComponents": []
  },
  {
    "id": "api.create-step",
    "service": "kanban.v1.KanbanService",
    "method": "CreateStep",
    "path": "/kanban.v1.KanbanService/CreateStep",
    "request": "CreateStepRequest",
    "response": "CreateStepResponse",
    "serverStreaming": false,
    "obligations": {
      "path": {
        "kind": "contract",
        "contractId": "api.create-step.path"
      },
      "query": {
        "kind": "not_applicable"
      },
      "headers": {
        "kind": "contract",
        "contractId": "api.create-step.headers"
      },
      "body": {
        "kind": "contract",
        "contractId": "api.create-step.request"
      },
      "success": {
        "kind": "contract",
        "contractId": "api.create-step.response"
      }
    },
    "sharedComponents": []
  },
  {
    "id": "api.update-step",
    "service": "kanban.v1.KanbanService",
    "method": "UpdateStep",
    "path": "/kanban.v1.KanbanService/UpdateStep",
    "request": "UpdateStepRequest",
    "response": "UpdateStepResponse",
    "serverStreaming": false,
    "obligations": {
      "path": {
        "kind": "contract",
        "contractId": "api.update-step.path"
      },
      "query": {
        "kind": "not_applicable"
      },
      "headers": {
        "kind": "contract",
        "contractId": "api.update-step.headers"
      },
      "body": {
        "kind": "contract",
        "contractId": "api.update-step.request"
      },
      "success": {
        "kind": "contract",
        "contractId": "api.update-step.response"
      }
    },
    "sharedComponents": []
  },
  {
    "id": "api.remove-step",
    "service": "kanban.v1.KanbanService",
    "method": "RemoveStep",
    "path": "/kanban.v1.KanbanService/RemoveStep",
    "request": "RemoveStepRequest",
    "response": "RemoveStepResponse",
    "serverStreaming": false,
    "obligations": {
      "path": {
        "kind": "contract",
        "contractId": "api.remove-step.path"
      },
      "query": {
        "kind": "not_applicable"
      },
      "headers": {
        "kind": "contract",
        "contractId": "api.remove-step.headers"
      },
      "body": {
        "kind": "not_applicable"
      },
      "success": {
        "kind": "contract",
        "contractId": "api.remove-step.response"
      }
    },
    "sharedComponents": []
  },
  {
    "id": "api.complete-step",
    "service": "kanban.v1.KanbanService",
    "method": "CompleteStep",
    "path": "/kanban.v1.KanbanService/CompleteStep",
    "request": "CompleteStepRequest",
    "response": "CompleteStepResponse",
    "serverStreaming": false,
    "obligations": {
      "path": {
        "kind": "contract",
        "contractId": "api.complete-step.path"
      },
      "query": {
        "kind": "not_applicable"
      },
      "headers": {
        "kind": "contract",
        "contractId": "api.complete-step.headers"
      },
      "body": {
        "kind": "contract",
        "contractId": "api.complete-step.request"
      },
      "success": {
        "kind": "contract",
        "contractId": "api.complete-step.response"
      }
    },
    "sharedComponents": []
  },
  {
    "id": "api.skip-step",
    "service": "kanban.v1.KanbanService",
    "method": "SkipStep",
    "path": "/kanban.v1.KanbanService/SkipStep",
    "request": "SkipStepRequest",
    "response": "SkipStepResponse",
    "serverStreaming": false,
    "obligations": {
      "path": {
        "kind": "contract",
        "contractId": "api.skip-step.path"
      },
      "query": {
        "kind": "not_applicable"
      },
      "headers": {
        "kind": "contract",
        "contractId": "api.skip-step.headers"
      },
      "body": {
        "kind": "contract",
        "contractId": "api.skip-step.request"
      },
      "success": {
        "kind": "contract",
        "contractId": "api.skip-step.response"
      }
    },
    "sharedComponents": []
  },
  {
    "id": "api.reopen-step",
    "service": "kanban.v1.KanbanService",
    "method": "ReopenStep",
    "path": "/kanban.v1.KanbanService/ReopenStep",
    "request": "ReopenStepRequest",
    "response": "ReopenStepResponse",
    "serverStreaming": false,
    "obligations": {
      "path": {
        "kind": "contract",
        "contractId": "api.reopen-step.path"
      },
      "query": {
        "kind": "not_applicable"
      },
      "headers": {
        "kind": "contract",
        "contractId": "api.reopen-step.headers"
      },
      "body": {
        "kind": "contract",
        "contractId": "api.reopen-step.request"
      },
      "success": {
        "kind": "contract",
        "contractId": "api.reopen-step.response"
      }
    },
    "sharedComponents": []
  },
  {
    "id": "api.mark-execution-plan-not-required",
    "service": "kanban.v1.KanbanService",
    "method": "MarkExecutionPlanNotRequired",
    "path": "/kanban.v1.KanbanService/MarkExecutionPlanNotRequired",
    "request": "MarkExecutionPlanNotRequiredRequest",
    "response": "MarkExecutionPlanNotRequiredResponse",
    "serverStreaming": false,
    "obligations": {
      "path": {
        "kind": "contract",
        "contractId": "api.mark-execution-plan-not-required.path"
      },
      "query": {
        "kind": "not_applicable"
      },
      "headers": {
        "kind": "contract",
        "contractId": "api.mark-execution-plan-not-required.headers"
      },
      "body": {
        "kind": "contract",
        "contractId": "api.mark-execution-plan-not-required.request"
      },
      "success": {
        "kind": "contract",
        "contractId": "api.mark-execution-plan-not-required.response"
      }
    },
    "sharedComponents": []
  },
  {
    "id": "api.list-runs",
    "service": "kanban.v1.KanbanService",
    "method": "ListRuns",
    "path": "/kanban.v1.KanbanService/ListRuns",
    "request": "ListRunsRequest",
    "response": "ListRunsResponse",
    "serverStreaming": false,
    "obligations": {
      "path": {
        "kind": "contract",
        "contractId": "api.list-runs.path"
      },
      "query": {
        "kind": "not_applicable"
      },
      "headers": {
        "kind": "contract",
        "contractId": "api.list-runs.headers"
      },
      "body": {
        "kind": "not_applicable"
      },
      "success": {
        "kind": "contract",
        "contractId": "api.list-runs.response"
      }
    },
    "sharedComponents": [
      "api.error.response"
    ]
  },
  {
    "id": "api.get-run-log",
    "service": "kanban.v1.KanbanService",
    "method": "GetRunLog",
    "path": "/kanban.v1.KanbanService/GetRunLog",
    "request": "GetRunLogRequest",
    "response": "GetRunLogResponse",
    "serverStreaming": false,
    "obligations": {
      "path": {
        "kind": "contract",
        "contractId": "api.get-run-log.path"
      },
      "query": {
        "kind": "not_applicable"
      },
      "headers": {
        "kind": "contract",
        "contractId": "api.get-run-log.headers"
      },
      "body": {
        "kind": "not_applicable"
      },
      "success": {
        "kind": "contract",
        "contractId": "api.get-run-log.response"
      }
    },
    "sharedComponents": []
  },
  {
    "id": "api.list-comments",
    "service": "kanban.v1.KanbanService",
    "method": "ListComments",
    "path": "/kanban.v1.KanbanService/ListComments",
    "request": "ListCommentsRequest",
    "response": "ListCommentsResponse",
    "serverStreaming": false,
    "obligations": {
      "path": {
        "kind": "contract",
        "contractId": "api.list-comments.path"
      },
      "query": {
        "kind": "not_applicable"
      },
      "headers": {
        "kind": "contract",
        "contractId": "api.list-comments.headers"
      },
      "body": {
        "kind": "not_applicable"
      },
      "success": {
        "kind": "contract",
        "contractId": "api.list-comments.response"
      }
    },
    "sharedComponents": [
      "api.error.response"
    ]
  },
  {
    "id": "api.create-comment",
    "service": "kanban.v1.KanbanService",
    "method": "CreateComment",
    "path": "/kanban.v1.KanbanService/CreateComment",
    "request": "CreateCommentRequest",
    "response": "CreateCommentResponse",
    "serverStreaming": false,
    "obligations": {
      "path": {
        "kind": "contract",
        "contractId": "api.create-comment.path"
      },
      "query": {
        "kind": "not_applicable"
      },
      "headers": {
        "kind": "contract",
        "contractId": "api.create-comment.headers"
      },
      "body": {
        "kind": "contract",
        "contractId": "api.create-comment.request"
      },
      "success": {
        "kind": "contract",
        "contractId": "api.create-comment.response"
      }
    },
    "sharedComponents": [
      "api.error.response"
    ]
  },
  {
    "id": "api.list-attachments",
    "service": "kanban.v1.KanbanService",
    "method": "ListAttachments",
    "path": "/kanban.v1.KanbanService/ListAttachments",
    "request": "ListAttachmentsRequest",
    "response": "ListAttachmentsResponse",
    "serverStreaming": false,
    "obligations": {
      "path": {
        "kind": "contract",
        "contractId": "api.list-attachments.path"
      },
      "query": {
        "kind": "not_applicable"
      },
      "headers": {
        "kind": "contract",
        "contractId": "api.list-attachments.headers"
      },
      "body": {
        "kind": "not_applicable"
      },
      "success": {
        "kind": "contract",
        "contractId": "api.list-attachments.response"
      }
    },
    "sharedComponents": []
  },
  {
    "id": "api.create-attachment",
    "service": "kanban.v1.KanbanService",
    "method": "CreateAttachment",
    "path": "/kanban.v1.KanbanService/CreateAttachment",
    "request": "CreateAttachmentRequest",
    "response": "CreateAttachmentResponse",
    "serverStreaming": false,
    "obligations": {
      "path": {
        "kind": "contract",
        "contractId": "api.create-attachment.path"
      },
      "query": {
        "kind": "not_applicable"
      },
      "headers": {
        "kind": "contract",
        "contractId": "api.create-attachment.headers"
      },
      "body": {
        "kind": "contract",
        "contractId": "api.create-attachment.request"
      },
      "success": {
        "kind": "contract",
        "contractId": "api.create-attachment.response"
      }
    },
    "sharedComponents": []
  },
  {
    "id": "api.download-attachment",
    "service": "kanban.v1.KanbanService",
    "method": "DownloadAttachment",
    "path": "/kanban.v1.KanbanService/DownloadAttachment",
    "request": "DownloadAttachmentRequest",
    "response": "DownloadAttachmentResponse",
    "serverStreaming": false,
    "obligations": {
      "path": {
        "kind": "contract",
        "contractId": "api.download-attachment.path"
      },
      "query": {
        "kind": "not_applicable"
      },
      "headers": {
        "kind": "contract",
        "contractId": "api.download-attachment.headers"
      },
      "body": {
        "kind": "not_applicable"
      },
      "success": {
        "kind": "contract",
        "contractId": "api.download-attachment.response"
      }
    },
    "sharedComponents": []
  },
  {
    "id": "api.delete-attachment",
    "service": "kanban.v1.KanbanService",
    "method": "DeleteAttachment",
    "path": "/kanban.v1.KanbanService/DeleteAttachment",
    "request": "DeleteAttachmentRequest",
    "response": "DeleteAttachmentResponse",
    "serverStreaming": false,
    "obligations": {
      "path": {
        "kind": "contract",
        "contractId": "api.delete-attachment.path"
      },
      "query": {
        "kind": "not_applicable"
      },
      "headers": {
        "kind": "contract",
        "contractId": "api.delete-attachment.headers"
      },
      "body": {
        "kind": "not_applicable"
      },
      "success": {
        "kind": "contract",
        "contractId": "api.delete-attachment.response"
      }
    },
    "sharedComponents": []
  },
  {
    "id": "api.get-stats",
    "service": "kanban.v1.KanbanService",
    "method": "GetStats",
    "path": "/kanban.v1.KanbanService/GetStats",
    "request": "GetStatsRequest",
    "response": "GetStatsResponse",
    "serverStreaming": false,
    "obligations": {
      "path": {
        "kind": "not_applicable"
      },
      "query": {
        "kind": "contract",
        "contractId": "api.get-stats.query"
      },
      "headers": {
        "kind": "contract",
        "contractId": "api.get-stats.headers"
      },
      "body": {
        "kind": "not_applicable"
      },
      "success": {
        "kind": "contract",
        "contractId": "api.get-stats.response"
      }
    },
    "sharedComponents": []
  },
  {
    "id": "api.search-status",
    "service": "kanban.v1.KanbanService",
    "method": "SearchStatus",
    "path": "/kanban.v1.KanbanService/SearchStatus",
    "request": "SearchStatusRequest",
    "response": "SearchStatusResponse",
    "serverStreaming": false,
    "obligations": {
      "path": {
        "kind": "not_applicable"
      },
      "query": {
        "kind": "contract",
        "contractId": "api.search-status.query"
      },
      "headers": {
        "kind": "contract",
        "contractId": "api.search-status.headers"
      },
      "body": {
        "kind": "not_applicable"
      },
      "success": {
        "kind": "contract",
        "contractId": "api.search-status.response"
      }
    },
    "sharedComponents": []
  },
  {
    "id": "api.list-events",
    "service": "kanban.v1.KanbanService",
    "method": "ListEvents",
    "path": "/kanban.v1.KanbanService/ListEvents",
    "request": "ListEventsRequest",
    "response": "ListEventsResponse",
    "serverStreaming": false,
    "obligations": {
      "path": {
        "kind": "not_applicable"
      },
      "query": {
        "kind": "contract",
        "contractId": "api.list-events.query"
      },
      "headers": {
        "kind": "contract",
        "contractId": "api.list-events.headers"
      },
      "body": {
        "kind": "not_applicable"
      },
      "success": {
        "kind": "contract",
        "contractId": "api.list-events.response"
      }
    },
    "sharedComponents": [
      "api.event.data"
    ]
  },
  {
    "id": "api.doctor",
    "service": "kanban.v1.KanbanService",
    "method": "Doctor",
    "path": "/kanban.v1.KanbanService/Doctor",
    "request": "DoctorRequest",
    "response": "DoctorResponse",
    "serverStreaming": false,
    "obligations": {
      "path": {
        "kind": "not_applicable"
      },
      "query": {
        "kind": "not_applicable"
      },
      "headers": {
        "kind": "contract",
        "contractId": "api.doctor.headers"
      },
      "body": {
        "kind": "not_applicable"
      },
      "success": {
        "kind": "contract",
        "contractId": "api.doctor.response"
      }
    },
    "sharedComponents": []
  },
  {
    "id": "api.checkpoint",
    "service": "kanban.v1.KanbanService",
    "method": "Checkpoint",
    "path": "/kanban.v1.KanbanService/Checkpoint",
    "request": "CheckpointRequest",
    "response": "CheckpointResponse",
    "serverStreaming": false,
    "obligations": {
      "path": {
        "kind": "not_applicable"
      },
      "query": {
        "kind": "not_applicable"
      },
      "headers": {
        "kind": "contract",
        "contractId": "api.checkpoint.headers"
      },
      "body": {
        "kind": "not_applicable"
      },
      "success": {
        "kind": "contract",
        "contractId": "api.checkpoint.response"
      }
    },
    "sharedComponents": []
  },
  {
    "id": "api.maintenance-backup",
    "service": "kanban.v1.KanbanService",
    "method": "MaintenanceBackup",
    "path": "/kanban.v1.KanbanService/MaintenanceBackup",
    "request": "MaintenanceBackupRequest",
    "response": "MaintenanceBackupResponse",
    "serverStreaming": false,
    "obligations": {
      "path": {
        "kind": "not_applicable"
      },
      "query": {
        "kind": "not_applicable"
      },
      "headers": {
        "kind": "contract",
        "contractId": "api.maintenance-backup.headers"
      },
      "body": {
        "kind": "contract",
        "contractId": "api.maintenance-backup.request"
      },
      "success": {
        "kind": "contract",
        "contractId": "api.maintenance-backup.response"
      }
    },
    "sharedComponents": []
  },
  {
    "id": "api.maintenance-export",
    "service": "kanban.v1.KanbanService",
    "method": "MaintenanceExport",
    "path": "/kanban.v1.KanbanService/MaintenanceExport",
    "request": "MaintenanceExportRequest",
    "response": "MaintenanceExportResponse",
    "serverStreaming": false,
    "obligations": {
      "path": {
        "kind": "not_applicable"
      },
      "query": {
        "kind": "not_applicable"
      },
      "headers": {
        "kind": "contract",
        "contractId": "api.maintenance-export.headers"
      },
      "body": {
        "kind": "contract",
        "contractId": "api.maintenance-export.request"
      },
      "success": {
        "kind": "contract",
        "contractId": "api.maintenance-export.response"
      }
    },
    "sharedComponents": []
  },
  {
    "id": "api.maintenance-import",
    "service": "kanban.v1.KanbanService",
    "method": "MaintenanceImport",
    "path": "/kanban.v1.KanbanService/MaintenanceImport",
    "request": "MaintenanceImportRequest",
    "response": "MaintenanceImportResponse",
    "serverStreaming": false,
    "obligations": {
      "path": {
        "kind": "not_applicable"
      },
      "query": {
        "kind": "not_applicable"
      },
      "headers": {
        "kind": "contract",
        "contractId": "api.maintenance-import.headers"
      },
      "body": {
        "kind": "contract",
        "contractId": "api.maintenance-import.request"
      },
      "success": {
        "kind": "contract",
        "contractId": "api.maintenance-import.response"
      }
    },
    "sharedComponents": []
  },
  {
    "id": "api.maintenance-vacuum",
    "service": "kanban.v1.KanbanService",
    "method": "MaintenanceVacuum",
    "path": "/kanban.v1.KanbanService/MaintenanceVacuum",
    "request": "MaintenanceVacuumRequest",
    "response": "MaintenanceVacuumResponse",
    "serverStreaming": false,
    "obligations": {
      "path": {
        "kind": "not_applicable"
      },
      "query": {
        "kind": "not_applicable"
      },
      "headers": {
        "kind": "contract",
        "contractId": "api.maintenance-vacuum.headers"
      },
      "body": {
        "kind": "not_applicable"
      },
      "success": {
        "kind": "contract",
        "contractId": "api.maintenance-vacuum.response"
      }
    },
    "sharedComponents": []
  },
  {
    "id": "api.maintenance-status",
    "service": "kanban.v1.KanbanService",
    "method": "MaintenanceStatus",
    "path": "/kanban.v1.KanbanService/MaintenanceStatus",
    "request": "MaintenanceStatusRequest",
    "response": "MaintenanceStatusResponse",
    "serverStreaming": false,
    "obligations": {
      "path": {
        "kind": "not_applicable"
      },
      "query": {
        "kind": "not_applicable"
      },
      "headers": {
        "kind": "contract",
        "contractId": "api.maintenance-status.headers"
      },
      "body": {
        "kind": "not_applicable"
      },
      "success": {
        "kind": "contract",
        "contractId": "api.maintenance-status.response"
      }
    },
    "sharedComponents": []
  },
  {
    "id": "api.maintenance-run",
    "service": "kanban.v1.KanbanService",
    "method": "MaintenanceRun",
    "path": "/kanban.v1.KanbanService/MaintenanceRun",
    "request": "MaintenanceRunRequest",
    "response": "MaintenanceRunResponse",
    "serverStreaming": false,
    "obligations": {
      "path": {
        "kind": "not_applicable"
      },
      "query": {
        "kind": "not_applicable"
      },
      "headers": {
        "kind": "contract",
        "contractId": "api.maintenance-run.headers"
      },
      "body": {
        "kind": "contract",
        "contractId": "api.maintenance-run.request"
      },
      "success": {
        "kind": "contract",
        "contractId": "api.maintenance-run.response"
      }
    },
    "sharedComponents": []
  },
  {
    "id": "api.maintenance-rebuild",
    "service": "kanban.v1.KanbanService",
    "method": "MaintenanceRebuild",
    "path": "/kanban.v1.KanbanService/MaintenanceRebuild",
    "request": "MaintenanceRebuildRequest",
    "response": "MaintenanceRebuildResponse",
    "serverStreaming": false,
    "obligations": {
      "path": {
        "kind": "not_applicable"
      },
      "query": {
        "kind": "not_applicable"
      },
      "headers": {
        "kind": "contract",
        "contractId": "api.maintenance-rebuild.headers"
      },
      "body": {
        "kind": "contract",
        "contractId": "api.maintenance-rebuild.request"
      },
      "success": {
        "kind": "contract",
        "contractId": "api.maintenance-rebuild.response"
      }
    },
    "sharedComponents": []
  },
  {
    "id": "api.maintenance-cleanup",
    "service": "kanban.v1.KanbanService",
    "method": "MaintenanceCleanup",
    "path": "/kanban.v1.KanbanService/MaintenanceCleanup",
    "request": "MaintenanceCleanupRequest",
    "response": "MaintenanceCleanupResponse",
    "serverStreaming": false,
    "obligations": {
      "path": {
        "kind": "not_applicable"
      },
      "query": {
        "kind": "not_applicable"
      },
      "headers": {
        "kind": "contract",
        "contractId": "api.maintenance-cleanup.headers"
      },
      "body": {
        "kind": "contract",
        "contractId": "api.maintenance-cleanup.request"
      },
      "success": {
        "kind": "contract",
        "contractId": "api.maintenance-cleanup.response"
      }
    },
    "sharedComponents": []
  },
  {
    "id": "api.maintenance-import-v30",
    "service": "kanban.v1.KanbanService",
    "method": "MaintenanceImportV30",
    "path": "/kanban.v1.KanbanService/MaintenanceImportV30",
    "request": "MaintenanceImportV30Request",
    "response": "MaintenanceImportV30Response",
    "serverStreaming": false,
    "obligations": {
      "path": {
        "kind": "not_applicable"
      },
      "query": {
        "kind": "not_applicable"
      },
      "headers": {
        "kind": "contract",
        "contractId": "api.maintenance-import-v30.headers"
      },
      "body": {
        "kind": "contract",
        "contractId": "api.maintenance-import-v30.request"
      },
      "success": {
        "kind": "contract",
        "contractId": "api.maintenance-import-v30.response"
      }
    },
    "sharedComponents": []
  }
] as const;

export type WebOperation = (typeof operations)[number];
export type WebOperationId = WebOperation["id"];

export const operationById = {
  "api.health": operations[0],
  "api.list-boards": operations[1],
  "api.list-board-columns": operations[2],
  "api.explain-label-atom": operations[3],
  "api.list-tasks": operations[4],
  "api.list-tasks-by-status": operations[5],
  "api.create-task": operations[6],
  "api.review-signals": operations[7],
  "api.get-signal": operations[8],
  "api.board-task-map": operations[9],
  "api.get-task": operations[10],
  "api.update-task": operations[11],
  "api.task-neighborhood": operations[12],
  "api.list-task-labels": operations[13],
  "api.add-task-label": operations[14],
  "api.suggest-task-labels": operations[15],
  "api.list-label-ontology-signals": operations[16],
  "api.review-label-ontology": operations[17],
  "api.create-label-ontology-action": operations[18],
  "api.get-label-ontology-signal": operations[19],
  "api.remove-task-label": operations[20],
  "api.specify-task": operations[21],
  "api.promote-task": operations[22],
  "api.claim-task": operations[23],
  "api.heartbeat-task": operations[24],
  "api.complete-task": operations[25],
  "api.submit-review-task": operations[26],
  "api.block-task": operations[27],
  "api.unblock-task": operations[28],
  "api.archive-task": operations[29],
  "api.list-dependencies": operations[30],
  "api.add-dependency": operations[31],
  "api.remove-dependency": operations[32],
  "api.list-steps": operations[33],
  "api.create-step": operations[34],
  "api.update-step": operations[35],
  "api.remove-step": operations[36],
  "api.complete-step": operations[37],
  "api.skip-step": operations[38],
  "api.reopen-step": operations[39],
  "api.mark-execution-plan-not-required": operations[40],
  "api.list-runs": operations[41],
  "api.get-run-log": operations[42],
  "api.list-comments": operations[43],
  "api.create-comment": operations[44],
  "api.list-attachments": operations[45],
  "api.create-attachment": operations[46],
  "api.download-attachment": operations[47],
  "api.delete-attachment": operations[48],
  "api.get-stats": operations[49],
  "api.search-status": operations[50],
  "api.list-events": operations[51],
  "api.doctor": operations[52],
  "api.checkpoint": operations[53],
  "api.maintenance-backup": operations[54],
  "api.maintenance-export": operations[55],
  "api.maintenance-import": operations[56],
  "api.maintenance-vacuum": operations[57],
  "api.maintenance-status": operations[58],
  "api.maintenance-run": operations[59],
  "api.maintenance-rebuild": operations[60],
  "api.maintenance-cleanup": operations[61],
  "api.maintenance-import-v30": operations[62],
} as const;

export function getOperation<K extends WebOperationId>(id: K): (typeof operationById)[K] {
  return operationById[id];
}
