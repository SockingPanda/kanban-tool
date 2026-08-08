// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
export const operations = [
  {
    "id": "api.health",
    "method": "GET",
    "path": "/health",
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
      },
      "sse": {
        "kind": "not_applicable"
      }
    },
    "sharedComponents": []
  },
  {
    "id": "api.list-boards",
    "method": "GET",
    "path": "/api/v1/boards",
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
      },
      "sse": {
        "kind": "not_applicable"
      }
    },
    "sharedComponents": []
  },
  {
    "id": "api.list-board-columns",
    "method": "GET",
    "path": "/api/v1/boards/:board/columns",
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
      },
      "sse": {
        "kind": "not_applicable"
      }
    },
    "sharedComponents": []
  },
  {
    "id": "api.explain-label-atom",
    "method": "GET",
    "path": "/api/v1/boards/:board/labels/atoms/:atom_ref/explain",
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
      },
      "sse": {
        "kind": "not_applicable"
      }
    },
    "sharedComponents": []
  },
  {
    "id": "api.list-tasks",
    "method": "GET",
    "path": "/api/v1/boards/:board/tasks",
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
      },
      "sse": {
        "kind": "not_applicable"
      }
    },
    "sharedComponents": [
      "api.error.response"
    ]
  },
  {
    "id": "api.list-tasks-by-status",
    "method": "GET",
    "path": "/api/v1/boards/:board/tasks/by-status",
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
      },
      "sse": {
        "kind": "not_applicable"
      }
    },
    "sharedComponents": [
      "api.error.response"
    ]
  },
  {
    "id": "api.create-task",
    "method": "POST",
    "path": "/api/v1/boards/:board/tasks",
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
      },
      "sse": {
        "kind": "not_applicable"
      }
    },
    "sharedComponents": []
  },
  {
    "id": "api.review-signals",
    "method": "GET",
    "path": "/api/v1/boards/:board/signals/review",
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
      },
      "sse": {
        "kind": "not_applicable"
      }
    },
    "sharedComponents": []
  },
  {
    "id": "api.get-signal",
    "method": "GET",
    "path": "/api/v1/signals/:signal_id",
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
      },
      "sse": {
        "kind": "not_applicable"
      }
    },
    "sharedComponents": []
  },
  {
    "id": "api.board-task-map",
    "method": "GET",
    "path": "/api/v1/boards/:board/task-map",
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
      },
      "sse": {
        "kind": "not_applicable"
      }
    },
    "sharedComponents": []
  },
  {
    "id": "api.get-task",
    "method": "GET",
    "path": "/api/v1/tasks/:task_id",
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
      },
      "sse": {
        "kind": "not_applicable"
      }
    },
    "sharedComponents": []
  },
  {
    "id": "api.update-task",
    "method": "PATCH",
    "path": "/api/v1/tasks/:task_id",
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
      },
      "sse": {
        "kind": "not_applicable"
      }
    },
    "sharedComponents": []
  },
  {
    "id": "api.task-neighborhood",
    "method": "GET",
    "path": "/api/v1/tasks/:task_id/neighborhood",
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
      },
      "sse": {
        "kind": "not_applicable"
      }
    },
    "sharedComponents": []
  },
  {
    "id": "api.add-task-label",
    "method": "POST",
    "path": "/api/v1/tasks/:task_id/labels",
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
      },
      "sse": {
        "kind": "not_applicable"
      }
    },
    "sharedComponents": [
      "api.error.response"
    ]
  },
  {
    "id": "api.suggest-task-labels",
    "method": "GET",
    "path": "/api/v1/tasks/:task_id/labels/suggestions",
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
      },
      "sse": {
        "kind": "not_applicable"
      }
    },
    "sharedComponents": []
  },
  {
    "id": "api.list-label-ontology-signals",
    "method": "GET",
    "path": "/api/v1/boards/:board/label-ontology/signals",
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
      },
      "sse": {
        "kind": "not_applicable"
      }
    },
    "sharedComponents": []
  },
  {
    "id": "api.review-label-ontology",
    "method": "GET",
    "path": "/api/v1/boards/:board/label-ontology/review",
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
      },
      "sse": {
        "kind": "not_applicable"
      }
    },
    "sharedComponents": []
  },
  {
    "id": "api.create-label-ontology-action",
    "method": "POST",
    "path": "/api/v1/boards/:board/label-ontology/actions",
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
      },
      "sse": {
        "kind": "not_applicable"
      }
    },
    "sharedComponents": []
  },
  {
    "id": "api.get-label-ontology-signal",
    "method": "GET",
    "path": "/api/v1/label-ontology/signals/:signal_id",
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
      },
      "sse": {
        "kind": "not_applicable"
      }
    },
    "sharedComponents": []
  },
  {
    "id": "api.remove-task-label",
    "method": "DELETE",
    "path": "/api/v1/tasks/:task_id/labels/:label_id",
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
      },
      "sse": {
        "kind": "not_applicable"
      }
    },
    "sharedComponents": [
      "api.error.response"
    ]
  },
  {
    "id": "api.specify-task",
    "method": "POST",
    "path": "/api/v1/tasks/:task_id/transitions/specify",
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
      },
      "sse": {
        "kind": "not_applicable"
      }
    },
    "sharedComponents": []
  },
  {
    "id": "api.promote-task",
    "method": "POST",
    "path": "/api/v1/tasks/:task_id/transitions/promote",
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
      },
      "sse": {
        "kind": "not_applicable"
      }
    },
    "sharedComponents": []
  },
  {
    "id": "api.claim-task",
    "method": "POST",
    "path": "/api/v1/tasks/:task_id/transitions/claim",
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
      },
      "sse": {
        "kind": "not_applicable"
      }
    },
    "sharedComponents": []
  },
  {
    "id": "api.heartbeat-task",
    "method": "POST",
    "path": "/api/v1/tasks/:task_id/transitions/heartbeat",
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
      },
      "sse": {
        "kind": "not_applicable"
      }
    },
    "sharedComponents": []
  },
  {
    "id": "api.complete-task",
    "method": "POST",
    "path": "/api/v1/tasks/:task_id/transitions/complete",
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
      },
      "sse": {
        "kind": "not_applicable"
      }
    },
    "sharedComponents": []
  },
  {
    "id": "api.submit-review-task",
    "method": "POST",
    "path": "/api/v1/tasks/:task_id/transitions/submit-review",
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
      },
      "sse": {
        "kind": "not_applicable"
      }
    },
    "sharedComponents": []
  },
  {
    "id": "api.block-task",
    "method": "POST",
    "path": "/api/v1/tasks/:task_id/transitions/block",
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
      },
      "sse": {
        "kind": "not_applicable"
      }
    },
    "sharedComponents": []
  },
  {
    "id": "api.unblock-task",
    "method": "POST",
    "path": "/api/v1/tasks/:task_id/transitions/unblock",
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
      },
      "sse": {
        "kind": "not_applicable"
      }
    },
    "sharedComponents": []
  },
  {
    "id": "api.archive-task",
    "method": "POST",
    "path": "/api/v1/tasks/:task_id/transitions/archive",
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
      },
      "sse": {
        "kind": "not_applicable"
      }
    },
    "sharedComponents": []
  },
  {
    "id": "api.list-dependencies",
    "method": "GET",
    "path": "/api/v1/tasks/:task_id/dependencies",
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
      },
      "sse": {
        "kind": "not_applicable"
      }
    },
    "sharedComponents": []
  },
  {
    "id": "api.add-dependency",
    "method": "POST",
    "path": "/api/v1/tasks/:task_id/dependencies",
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
      },
      "sse": {
        "kind": "not_applicable"
      }
    },
    "sharedComponents": []
  },
  {
    "id": "api.remove-dependency",
    "method": "DELETE",
    "path": "/api/v1/tasks/:child_task_id/dependencies/:parent_task_id",
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
      },
      "sse": {
        "kind": "not_applicable"
      }
    },
    "sharedComponents": []
  },
  {
    "id": "api.list-steps",
    "method": "GET",
    "path": "/api/v1/tasks/:task_id/steps",
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
      },
      "sse": {
        "kind": "not_applicable"
      }
    },
    "sharedComponents": []
  },
  {
    "id": "api.create-step",
    "method": "POST",
    "path": "/api/v1/tasks/:task_id/steps",
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
      },
      "sse": {
        "kind": "not_applicable"
      }
    },
    "sharedComponents": []
  },
  {
    "id": "api.mark-execution-plan-not-required",
    "method": "POST",
    "path": "/api/v1/tasks/:task_id/execution-plan/not-required",
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
      },
      "sse": {
        "kind": "not_applicable"
      }
    },
    "sharedComponents": []
  },
  {
    "id": "api.list-runs",
    "method": "GET",
    "path": "/api/v1/tasks/:task_id/runs",
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
      },
      "sse": {
        "kind": "not_applicable"
      }
    },
    "sharedComponents": [
      "api.error.response"
    ]
  },
  {
    "id": "api.get-run-log",
    "method": "GET",
    "path": "/api/v1/runs/:run_id/log",
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
      },
      "sse": {
        "kind": "not_applicable"
      }
    },
    "sharedComponents": []
  },
  {
    "id": "api.list-comments",
    "method": "GET",
    "path": "/api/v1/tasks/:task_id/comments",
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
      },
      "sse": {
        "kind": "not_applicable"
      }
    },
    "sharedComponents": [
      "api.error.response"
    ]
  },
  {
    "id": "api.create-comment",
    "method": "POST",
    "path": "/api/v1/tasks/:task_id/comments",
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
      },
      "sse": {
        "kind": "not_applicable"
      }
    },
    "sharedComponents": [
      "api.error.response"
    ]
  },
  {
    "id": "api.list-attachments",
    "method": "GET",
    "path": "/api/v1/tasks/:task_id/attachments",
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
      },
      "sse": {
        "kind": "not_applicable"
      }
    },
    "sharedComponents": []
  },
  {
    "id": "api.create-attachment",
    "method": "POST",
    "path": "/api/v1/tasks/:task_id/attachments",
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
      },
      "sse": {
        "kind": "not_applicable"
      }
    },
    "sharedComponents": []
  },
  {
    "id": "api.download-attachment",
    "method": "GET",
    "path": "/api/v1/tasks/:task_id/attachments/:attachment_id",
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
      },
      "sse": {
        "kind": "not_applicable"
      }
    },
    "sharedComponents": []
  },
  {
    "id": "api.delete-attachment",
    "method": "DELETE",
    "path": "/api/v1/tasks/:task_id/attachments/:attachment_id",
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
      },
      "sse": {
        "kind": "not_applicable"
      }
    },
    "sharedComponents": []
  },
  {
    "id": "api.get-stats",
    "method": "GET",
    "path": "/api/v1/stats",
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
      },
      "sse": {
        "kind": "not_applicable"
      }
    },
    "sharedComponents": []
  },
  {
    "id": "api.search-status",
    "method": "GET",
    "path": "/api/v1/search/status",
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
      },
      "sse": {
        "kind": "not_applicable"
      }
    },
    "sharedComponents": []
  },
  {
    "id": "api.list-events",
    "method": "GET",
    "path": "/api/v1/events",
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
      },
      "sse": {
        "kind": "not_applicable"
      }
    },
    "sharedComponents": []
  },
  {
    "id": "sse.stream-events",
    "method": "GET",
    "path": "/api/v1/stream/events",
    "obligations": {
      "path": {
        "kind": "not_applicable"
      },
      "query": {
        "kind": "contract",
        "contractId": "sse.stream-events.query"
      },
      "headers": {
        "kind": "contract",
        "contractId": "sse.stream-events.headers"
      },
      "body": {
        "kind": "not_applicable"
      },
      "success": {
        "kind": "not_applicable"
      },
      "sse": {
        "kind": "contract",
        "contractId": "sse.event.data"
      }
    },
    "sharedComponents": [
      "sse.event.heartbeat"
    ]
  },
  {
    "id": "api.doctor",
    "method": "GET",
    "path": "/api/v1/maintenance/doctor",
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
      },
      "sse": {
        "kind": "not_applicable"
      }
    },
    "sharedComponents": []
  },
  {
    "id": "api.checkpoint",
    "method": "POST",
    "path": "/api/v1/maintenance/checkpoint",
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
      },
      "sse": {
        "kind": "not_applicable"
      }
    },
    "sharedComponents": []
  },
  {
    "id": "api.maintenance-backup",
    "method": "POST",
    "path": "/api/v1/maintenance/backup",
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
      },
      "sse": {
        "kind": "not_applicable"
      }
    },
    "sharedComponents": []
  },
  {
    "id": "api.maintenance-export",
    "method": "POST",
    "path": "/api/v1/maintenance/export",
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
      },
      "sse": {
        "kind": "not_applicable"
      }
    },
    "sharedComponents": []
  },
  {
    "id": "api.maintenance-import",
    "method": "POST",
    "path": "/api/v1/maintenance/import",
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
      },
      "sse": {
        "kind": "not_applicable"
      }
    },
    "sharedComponents": []
  },
  {
    "id": "api.maintenance-vacuum",
    "method": "POST",
    "path": "/api/v1/maintenance/vacuum",
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
      },
      "sse": {
        "kind": "not_applicable"
      }
    },
    "sharedComponents": []
  },
  {
    "id": "api.maintenance-status",
    "method": "GET",
    "path": "/api/v1/maintenance/status",
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
      },
      "sse": {
        "kind": "not_applicable"
      }
    },
    "sharedComponents": []
  },
  {
    "id": "api.maintenance-run",
    "method": "POST",
    "path": "/api/v1/maintenance/run",
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
      },
      "sse": {
        "kind": "not_applicable"
      }
    },
    "sharedComponents": []
  },
  {
    "id": "api.maintenance-rebuild",
    "method": "POST",
    "path": "/api/v1/maintenance/rebuild",
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
      },
      "sse": {
        "kind": "not_applicable"
      }
    },
    "sharedComponents": []
  },
  {
    "id": "api.maintenance-cleanup",
    "method": "POST",
    "path": "/api/v1/maintenance/cleanup",
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
      },
      "sse": {
        "kind": "not_applicable"
      }
    },
    "sharedComponents": []
  },
  {
    "id": "api.maintenance-import-v30",
    "method": "POST",
    "path": "/api/v1/maintenance/import-v30",
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
      },
      "sse": {
        "kind": "not_applicable"
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
  "api.add-task-label": operations[13],
  "api.suggest-task-labels": operations[14],
  "api.list-label-ontology-signals": operations[15],
  "api.review-label-ontology": operations[16],
  "api.create-label-ontology-action": operations[17],
  "api.get-label-ontology-signal": operations[18],
  "api.remove-task-label": operations[19],
  "api.specify-task": operations[20],
  "api.promote-task": operations[21],
  "api.claim-task": operations[22],
  "api.heartbeat-task": operations[23],
  "api.complete-task": operations[24],
  "api.submit-review-task": operations[25],
  "api.block-task": operations[26],
  "api.unblock-task": operations[27],
  "api.archive-task": operations[28],
  "api.list-dependencies": operations[29],
  "api.add-dependency": operations[30],
  "api.remove-dependency": operations[31],
  "api.list-steps": operations[32],
  "api.create-step": operations[33],
  "api.mark-execution-plan-not-required": operations[34],
  "api.list-runs": operations[35],
  "api.get-run-log": operations[36],
  "api.list-comments": operations[37],
  "api.create-comment": operations[38],
  "api.list-attachments": operations[39],
  "api.create-attachment": operations[40],
  "api.download-attachment": operations[41],
  "api.delete-attachment": operations[42],
  "api.get-stats": operations[43],
  "api.search-status": operations[44],
  "api.list-events": operations[45],
  "sse.stream-events": operations[46],
  "api.doctor": operations[47],
  "api.checkpoint": operations[48],
  "api.maintenance-backup": operations[49],
  "api.maintenance-export": operations[50],
  "api.maintenance-import": operations[51],
  "api.maintenance-vacuum": operations[52],
  "api.maintenance-status": operations[53],
  "api.maintenance-run": operations[54],
  "api.maintenance-rebuild": operations[55],
  "api.maintenance-cleanup": operations[56],
  "api.maintenance-import-v30": operations[57],
} as const;

export function getOperation<K extends WebOperationId>(id: K): (typeof operationById)[K] {
  return operationById[id];
}
