# Ontology Closure Contract

This contract fixes the implementation boundary for the ontology closure work.
It is intentionally narrower than the historical behavior in some adapters.

Review baseline: `87c0949`.

## Scope

The closure line is:

- `labels` is the board-scoped vocabulary identity registry.
- `task_labels` is the task binding table.
- `label_semantics` plus `label_atoms` is the canonical label semantics surface.
- `label_ontology_actions` is the append-only provenance ledger for semantics
  and atom mutations only.
- `label_ontology_action_atom_effects` records atom-level deltas for a root
  mutation action. Effects are limited to `added` and `removed`.

Base identity CRUD is not an ontology mutation. Task label binding is not an
ontology mutation. Semantics and atom changes are ontology mutations.

## Invariants

1. `label create` creates only an empty vocabulary identity. It writes a normal
   board event and no ontology action.
2. `label delete` deletes only an empty vocabulary identity. It must not
   implicitly delete semantics or atoms. `--force` may remove task bindings, but
   it must still refuse a label that has semantics or atoms.
3. `task create --label` and normal task label add bind only existing labels
   unless the explicit task-label add API uses `create_missing=true`.
4. `create_missing=true` creates label identity only. It must not create
   semantics, atoms, ontology actions, or validation records.
5. Every canonical `label_semantics` / `label_atoms` transaction writes at most
   one root ontology mutation action.
6. A no-op semantics or atom request writes no root action, no atom effects, and
   does not mark the label atom index dirty.
7. Root action `change_json` stores one before/after semantics snapshot. Atom
   explain granularity comes from atom effect rows, not duplicated sibling
   mutation actions.
8. Atom effects record only atoms that were actually added or removed by the
   transaction.
9. Source signals must match the actual mutation semantics. Retarget override
   may change only the target label and must not relax action, polarity, or kind.
10. Do not add `LabelOntologyActionType`, `LabelOntologySignalKind`,
    `LabelOntologyProposedAction`, `validation_status`, or atom effect variants
    for this closure. Semantics clear uses `update_semantics`.

## Operation Matrix

| Operation | Canonical tables | Event | Root ontology action | CAS | Source-signal policy | Index dirty |
| --- | --- | --- | --- | --- | --- | --- |
| Create task, no labels | `tasks`; optional `task_dependencies`; `task_events` | `task.created` plus dependency events as applicable | None | Task creation has no semantics CAS | No ontology source signals accepted | No |
| Create task with labels | `tasks`, `task_labels`; optional `task_dependencies`; `task_events` | `task.created` and one `task.label.added` event per new binding | None | All label refs must resolve before task insert; any missing label aborts whole transaction | No ontology source signals accepted | No |
| Create label identity | `labels`, `task_events` | `label.created` | None | Idempotent by board/name; no semantics CAS | No ontology source signals accepted | No |
| Delete label identity | `labels`; optionally `task_labels`; `task_events` | `label.deleted`; task-label removal events when bindings are removed | None | Must verify no `label_semantics` row and no `label_atoms` rows before delete | No ontology source signals accepted | No |
| Add task label binding | `task_labels`, `task_events`; optionally `labels` only when explicit `create_missing=true` | `task.label.added`; `label.created` only for explicit identity creation | None | All label names are validated before any binding; default missing label is invalid input | No ontology source signals accepted | No |
| Remove task label binding | `task_labels`, `task_events` | `task.label.removed` when a binding existed | None | No semantics CAS | No ontology source signals accepted | No |
| Upsert/patch/replace semantics | `label_semantics`, `label_atoms`, `label_atom_index_boards`, `label_ontology_actions`, `label_ontology_action_atom_effects`, `label_ontology_action_signals` | None, unless an existing API already has a non-ontology task event for a separate task operation | One `update_semantics` root action if before/after semantics hash changes | `expected_semantics_hash` is required when supplied by the caller and must match current semantics hash | `update_semantics` signals may support any real semantics hash change; `add_positive_atom` requires a positive added effect; `add_negative_atom` requires a negative added effect; any incompatible signal aborts the transaction | Yes only when before/after semantics hash changes |
| Clear semantics | `label_semantics`, `label_atoms`, `label_atom_index_boards`, `label_ontology_actions`, `label_ontology_action_atom_effects`, `label_ontology_action_signals` | None | One `update_semantics` root action with after snapshot equal to empty semantics; removed atom effects for actual removed atoms | Required `expected_semantics_hash`; mismatch aborts without canonical, action, effect, or dirty writes | Same compatibility rules as semantics update; source signals are optional, but if present every signal must be compatible | Yes only when semantics or atoms existed |
| Direct bootstrap label semantics | `labels`, `task_labels`, `label_semantics`, `label_atoms`, `label_atom_index_boards`, `label_ontology_actions`, `label_ontology_action_atom_effects`, `label_ontology_action_signals` | `label.created` if identity was created; `task.label.added` when the task binding is created | One `bootstrap_label` root action with added effects for actual atoms | Requires the target label to have no existing semantics; validates target state in the same transaction | Requires confirmed same-board vocabulary-gap/bootstrap-label signals when source signals are supplied; retarget override records reason and original target/proposed label but does not relax the signal contract | Yes when semantics/atoms are created |
| Proposal accept | `label_semantic_proposals`, `labels`, `label_semantics`, `label_atoms`, `label_atom_index_boards`, `label_ontology_actions`, `label_ontology_action_atom_effects`, `label_ontology_action_signals` | `task.label_proposal.accepted`; `label.created` if identity was created | One `bootstrap_label` root action; `parent_action_id` points to the unique `create_label_proposal` action when one exists | Proposal must still be `proposed`; target label identity/semantics state is rechecked in transaction | Confirmed same-board vocabulary-gap/bootstrap-label signals only; normalized proposed label must match unless an explicit retarget override records the exception | Yes when semantics/atoms are created |
| Apply new positive/negative atom | `label_semantics`, `label_atoms`, `label_atom_index_boards`, `label_ontology_actions`, `label_ontology_action_atom_effects`, `label_ontology_action_signals` | None | One `add_positive_atom` or `add_negative_atom` root action with one `added` effect | Current label semantics is read and updated in one transaction; source signal validation and canonical write commit or roll back together | `add_positive_atom` requires confirmed positive candidate signal with matching action and kind; `add_negative_atom` requires confirmed negative candidate signal with matching action and kind; text may be generalized | Yes only when the atom is newly added |
| Adopt existing atom | `label_ontology_actions`, `label_ontology_action_signals` | None | One `adopt_existing_atom` provenance-only action; no atom effects | Before and after semantics hash must be identical | Same action, polarity, kind, board, status, and retarget rules as apply new atom | No |
| Revert root mutation | `label_semantics`, `label_atoms`, `label_atom_index_boards`, `label_ontology_actions`, `label_ontology_action_atom_effects`, `label_ontology_action_signals` | None | One `revert_ontology_mutation` root action whose parent is the reverted root action; effects describe this revert's actual added/removed atoms | Current semantics hash must equal the reverted root action's `canonical_after_hash`; optional `expected_current_hash` must also match | Revert copies source signal links from the target root action for provenance only; it does not revalidate signal compatibility as a new proposed mutation | Yes when the revert changes semantics/atoms |

## Compatibility Rules

Historical per-atom actions remain readable. New writes must prefer root actions
and atom effect rows. Atom explain reads new effect rows first, then falls back
to legacy `result_atom_id` / `result_atom_content_hash` matching with an
explicit legacy match reason.

Legacy action rows are not rewritten, compacted, or backfilled in this closure.

## Public Surface Rules

- Generic ontology action endpoints may write only lifecycle actions:
  `confirm`, `reject`, `supersede`, and `resolve_no_change`.
- Canonical semantics/atom mutation actions are written only by dedicated
  service paths in the same transaction as the canonical write.
- Raw trusted validation evidence ingestion is not a public API. External
  adapters can submit only external attestation; CLI trusted validation must run
  the real collector entry.
- Root mutation recorder and atom effect writer stay `pub(crate)`.
- Structure plan actions are deferred and must not gain new public write paths.

## Documentation Boundary

User-facing docs and the global `kanban-tool` skill are synchronized only after
the implementation behavior stabilizes. Until then, this contract is the
engineering source of truth for the closure tasks.
