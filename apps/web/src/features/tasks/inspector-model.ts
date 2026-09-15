export type InspectorTaskStatus = "triage" | "todo" | "scheduled" | "ready" | "running" | "blocked" | "review" | "done" | "archived"

export type InspectorPlanState = "unplanned" | "planned" | "not_required"

export interface TaskInspectorViewModel {
  readonly task: {
    readonly id: string
    readonly ref: string
    readonly title: string
    readonly status: InspectorTaskStatus
    /** UI-only optimistic concurrency inputs; canonical values come from the read mapper. */
    readonly lockVersion?: number
    readonly scheduledAt?: number | null
    readonly dueAt?: number | null
    readonly priority: number
    readonly description: string | null
    readonly statusReason: string | null
    readonly assignee: string | null
    readonly executionPlanState: InspectorPlanState
    readonly dependencyBlocked: boolean
    readonly unfinishedParentCount: number
    readonly requiredStepCount: number
    readonly completedRequiredStepCount: number
    readonly optionalStepCount: number
    readonly metadata: unknown
    readonly claimOwner: string | null
    readonly claimExpiresAt: number | null
    readonly lastHeartbeatAt: number | null
    readonly currentRunId: string | null
    readonly retryCount: number
    readonly maxRetries: number | null
    readonly createdAt: number
    readonly updatedAt: number
  }
  readonly steps: readonly {
    readonly id: string
    readonly title: string
    readonly status: "todo" | "done" | "skipped"
    readonly required: boolean
    readonly body: string | null
  }[]
  readonly parents: readonly InspectorDependency[]
  readonly children: readonly InspectorDependency[]
  readonly comments: readonly {
    readonly id: string
    readonly author: string
    readonly kind: "note" | "decision" | "signal"
    readonly body: string
    readonly createdAt: number
    readonly metadata?: Readonly<Record<string, unknown>>
  }[]
  readonly runs: readonly {
    readonly id: string
    readonly status: "running" | "succeeded" | "failed" | "canceled" | "expired"
    readonly workerProfile: string | null
    readonly claimOwner: string
    readonly startedAt: number
    readonly finishedAt: number | null
    readonly exitCode: number | null
    readonly error: string | null
    readonly hasLog: boolean
  }[]
  readonly events: readonly {
    readonly id: number
    readonly kind: string
    readonly actor: string | null
    readonly createdAt: number
  }[]
  readonly neighborhood?: {
    readonly centerTaskId: string
    readonly nodes: readonly { readonly id: string; readonly ref: string; readonly title: string; readonly role: string }[]
    readonly edges: readonly { readonly id: string; readonly sourceTaskId: string; readonly targetTaskId: string; readonly kind: string }[]
  }
  readonly runtime: {
    readonly actor: string
    readonly apiBaseUrl: string
    readonly serverVersion: string
    readonly protocolVersion: string
    readonly webBuildId: string
  }
}

export interface InspectorDependency {
  readonly id: string
  readonly ref: string
  readonly title: string
  readonly status: InspectorTaskStatus
}

export type InspectorCopy = {
  readonly ariaLabel: string
  readonly eyebrow: string
  readonly dependencyBlocked: string
  readonly sections: {
    readonly metadata: string
    readonly claim: string
    readonly steps: string
    readonly dependencies: string
    readonly comments: string
    readonly runs: string
    readonly events: string
    readonly neighborhood: string
    readonly runtime: string
  }
  readonly facts: {
    readonly statusReason: string
    readonly assignee: string
    readonly plan: string
    readonly requiredSteps: string
    readonly optionalSteps: string
    readonly createdAt: string
    readonly updatedAt: string
    readonly claimOwner: string
    readonly claimExpires: string
    readonly heartbeat: string
    readonly currentRun: string
    readonly retry: string
    readonly blockedParents: string
    readonly actor: string
    readonly api: string
    readonly server: string
    readonly protocol: string
    readonly build: string
  }
  readonly parents: string
  readonly children: string
  readonly description: string
  readonly showDescription: string
  readonly noDescription: string
  readonly noItems: string
  readonly noSteps: string
  readonly noComments: string
  readonly noRuns: string
  readonly noEvents: string
  readonly noNeighborhood: string
  readonly neighborhoodNodes: string
  readonly neighborhoodEdges: string
  readonly required: string
  readonly manual: string
  readonly system: string
  readonly log: string
  readonly loading: string
  readonly loadError: string
  readonly retry: string
  readonly openAnnouncement: string
  readonly refreshError: string
  readonly refreshOffline: string
  readonly refreshPending: string
  readonly offline: string
  readonly edit: string
  readonly save: string
  readonly saving: string
  readonly cancel: string
  readonly unsavedChanges: string
  readonly editTitle: string
  readonly editDescription: string
  readonly editAssignee: string
  readonly editPriority: string
  readonly editScheduledAt: string
  readonly editDueAt: string
  readonly actions: string
  readonly actionPending: string
  readonly actionRunning: string
  readonly actionConfirmTitle: string
  readonly actionConfirmDescription: string
  readonly actionDescriptionTitle: string
  readonly actionDescriptionHint: string
  readonly actionReasonTitle: string
  readonly actionReasonHint: string
  readonly actionForceConfirmation: string
  readonly reasonRequired: string
  readonly confirmationRequired: string
  readonly descriptionRequired: string
  readonly retryAction: string
  readonly mutationError: string
  readonly mutationRetrying: string
  readonly actionReasons: {
    readonly description: string
    readonly dependencies: string
    readonly plan: string
    readonly promote: string
    readonly claim: string
    readonly requiredSteps: string
    readonly status: string
  }
  readonly status: Readonly<Record<InspectorTaskStatus, string>>
  readonly planState: Readonly<Record<InspectorPlanState, string>>
  readonly stepStatus: Readonly<Record<"todo" | "done" | "skipped", string>>
  readonly runStatus: Readonly<Record<"running" | "succeeded" | "failed" | "canceled" | "expired", string>>
  readonly commentKind: Readonly<Record<"note" | "decision" | "signal", string>>
}
